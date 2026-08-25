//! `linear git review-url` — resolve the Linear review page for an issue's pull
//! requests.
//!
//! Linear exposes a pull request's review URL in two places, and neither covers
//! every pull request on its own:
//!
//! * a `PullRequestNotification` carries the review URL whole
//!   (`review/<title-slug>-<id>`), but only exists once the pull request has
//!   produced notification-worthy activity;
//! * an agent session's `PullRequest.slugId` is enough to assemble a review URL,
//!   but only pull requests an agent worked on have a session.
//!
//! So notifications are the primary source, agent sessions the fallback, and a
//! pull request neither source resolves is reported as unresolved rather than
//! guessed at.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::api::LinearClient;
use crate::output::{print_json, OutputOptions};
use crate::pagination::{paginate_until, PageFlow, PaginationOptions};

/// How many notifications `review-url` reads before falling through to agent
/// sessions.
///
/// The feed has no server-side filter for pull requests, so it is walked
/// newest-first; a pull request whose last notification is older than this is
/// left to the agent-session path.
const NOTIFICATION_LIMIT: usize = 500;

/// Notifications per request while walking the feed.
const NOTIFICATION_PAGE_SIZE: usize = 100;

const ISSUE_QUERY: &str = r#"
    query($id: String!) {
        organization { urlKey }
        issue(id: $id) {
            identifier
            attachments { nodes { url sourceType } }
            agentSessions {
                nodes {
                    pullRequests {
                        nodes {
                            pullRequest { slugId url number status title }
                        }
                    }
                }
            }
        }
    }
"#;

const NOTIFICATIONS_QUERY: &str = r#"
    query($first: Int, $after: String) {
        notifications(first: $first, after: $after, includeArchived: true) {
            pageInfo { hasNextPage endCursor }
            nodes {
                __typename
                ... on PullRequestNotification {
                    url
                    pullRequest { url number status title }
                }
            }
        }
    }
"#;

/// A Linear GraphQL connection, reduced to the nodes callers care about.
#[derive(Debug, Deserialize)]
// `#[serde(default)]` on `nodes` would otherwise pull a `T: Default` bound into
// the generated impl; only `Deserialize` is actually needed.
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct NodeList<T> {
    #[serde(default)]
    nodes: Vec<T>,
}

impl<T> Default for NodeList<T> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Issue {
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    attachments: NodeList<Attachment>,
    #[serde(default)]
    agent_sessions: NodeList<AgentSession>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attachment {
    #[serde(default)]
    source_type: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentSession {
    #[serde(default)]
    pull_requests: NodeList<AgentSessionPullRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentSessionPullRequest {
    #[serde(default)]
    pull_request: Option<PullRequest>,
}

/// A pull request as Linear returns it, on either source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequest {
    #[serde(default)]
    slug_id: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    number: Option<i64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

/// A node from the notification feed. The feed is heterogeneous, so everything
/// but `__typename` is optional and non-pull-request nodes are dropped.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Notification {
    #[serde(rename = "__typename", default)]
    typename: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    pull_request: Option<PullRequest>,
}

/// One pull request that resolved to a review page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewEntry {
    review_url: String,
    number: Option<i64>,
    status: Option<String>,
    title: Option<String>,
    /// The GitHub pull request URL; the identity a merge keys on.
    url: String,
}

impl ReviewEntry {
    /// Build an entry for `pr` at an already-resolved `review_url`.
    ///
    /// A pull request Linear returned without a URL has no identity to merge or
    /// report on, so it yields nothing rather than an entry with a null `url`.
    fn new(review_url: String, pr: &PullRequest) -> Option<Self> {
        let url = pr.url.clone().filter(|u| !u.is_empty())?;
        Some(Self {
            review_url,
            number: pr.number,
            status: pr.status.clone(),
            title: pr.title.clone(),
            url,
        })
    }
}

/// The outcome of resolving an issue's pull requests.
///
/// Unresolved pull requests are part of the output contract, not a silent
/// omission: a caller that resolved two of three attached pull requests can see
/// which one it did not get.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Resolution {
    resolved: Vec<ReviewEntry>,
    /// GitHub pull request URLs attached to the issue that neither source resolved.
    unresolved: Vec<String>,
}

impl Resolution {
    /// Merge both sources into one entry per pull request.
    ///
    /// `fallback` (agent sessions) goes in first so a `primary` (notification)
    /// result — which carries the review URL whole rather than assembling it —
    /// replaces it. Entries are reported in `pr_urls` order, which is the order
    /// Linear lists the issue's attachments; a pull request an agent session
    /// linked without a corresponding attachment follows them.
    fn merge(pr_urls: &[String], fallback: Vec<ReviewEntry>, primary: Vec<ReviewEntry>) -> Self {
        let mut by_pr_url: BTreeMap<String, ReviewEntry> = BTreeMap::new();
        for entry in fallback.into_iter().chain(primary) {
            by_pr_url.insert(entry.url.clone(), entry);
        }

        let mut resolved = Vec::with_capacity(by_pr_url.len());
        let mut unresolved = Vec::new();
        for url in pr_urls {
            match by_pr_url.remove(url) {
                Some(entry) => resolved.push(entry),
                None => unresolved.push(url.clone()),
            }
        }
        resolved.extend(by_pr_url.into_values());

        Self {
            resolved,
            unresolved,
        }
    }
}

impl Issue {
    /// GitHub pull request URLs attached to the issue.
    ///
    /// Linear links a pull request to an issue as a `github` attachment as soon
    /// as it detects the branch, so this is the full set of the issue's pull
    /// requests — but the attachment carries no review slug, which is why the
    /// review URL is looked up separately.
    fn attached_pr_urls(&self) -> Vec<String> {
        let mut urls: Vec<String> = Vec::new();
        for attachment in &self.attachments.nodes {
            if attachment.source_type.as_deref() != Some("github") {
                continue;
            }
            let Some(url) = attachment.url.as_deref().filter(|u| u.contains("/pull/")) else {
                continue;
            };
            if !urls.iter().any(|existing| existing == url) {
                urls.push(url.to_string());
            }
        }
        urls
    }

    /// Review entries assembled from the issue's agent sessions.
    ///
    /// This is the fallback for a pull request with no notification: a session
    /// exposes `PullRequest.slugId`, from which the review URL can be built.
    fn agent_session_entries(&self, url_key: &str) -> Vec<ReviewEntry> {
        let mut entries = Vec::new();
        for session in &self.agent_sessions.nodes {
            for link in &session.pull_requests.nodes {
                let Some(pr) = link.pull_request.as_ref() else {
                    continue;
                };
                let Some(slug) = pr.slug_id.as_deref().filter(|s| !s.is_empty()) else {
                    continue;
                };
                let review_url = format!("https://linear.app/{}/review/{}", url_key, slug);
                if let Some(entry) = ReviewEntry::new(review_url, pr) {
                    entries.push(entry);
                }
            }
        }
        entries
    }
}

/// Pick out review entries for `pr_urls` from a page of notification nodes.
fn review_entries_from_notifications(nodes: &[Value], pr_urls: &[String]) -> Vec<ReviewEntry> {
    let mut entries = Vec::new();
    for node in nodes {
        let Ok(notification) = serde_json::from_value::<Notification>(node.clone()) else {
            continue;
        };
        if notification.typename.as_deref() != Some("PullRequestNotification") {
            continue;
        }
        let Some(pr) = notification.pull_request.as_ref() else {
            continue;
        };
        if !pr
            .url
            .as_deref()
            .is_some_and(|url| pr_urls.iter().any(|wanted| wanted == url))
        {
            continue;
        }
        let Some(review_url) = notification.url.as_deref().filter(|u| !u.is_empty()) else {
            continue;
        };
        // A comment notification points at an anchor within the review page
        // (`…#comment-<id>`); the page itself is what a caller wants.
        let review_url = review_url.split('#').next().unwrap_or(review_url);
        if let Some(entry) = ReviewEntry::new(review_url.to_string(), pr) {
            entries.push(entry);
        }
    }
    entries
}

/// Accumulates review entries across pages of the notification feed.
///
/// The feed is newest-first and a pull request can appear on it many times, so
/// the first entry seen for a pull request is its current one and later pages
/// must not displace it. Once every wanted pull request has an entry there is
/// nothing left to look for, which is what lets the walk stop early.
#[derive(Debug, Default)]
struct NotificationScan {
    entries: Vec<ReviewEntry>,
    seen_pr_urls: BTreeSet<String>,
}

impl NotificationScan {
    /// Take one page of notification nodes, and report whether to read another.
    fn absorb(&mut self, nodes: &[Value], pr_urls: &[String]) -> PageFlow {
        for entry in review_entries_from_notifications(nodes, pr_urls) {
            if self.seen_pr_urls.insert(entry.url.clone()) {
                self.entries.push(entry);
            }
        }

        if self.seen_pr_urls.len() >= pr_urls.len() {
            PageFlow::Stop
        } else {
            PageFlow::Continue
        }
    }
}

/// Walk the notification feed looking for the review URLs of `pr_urls`.
async fn notification_entries(
    client: &LinearClient,
    pr_urls: &[String],
) -> Result<Vec<ReviewEntry>> {
    let options = PaginationOptions {
        limit: Some(NOTIFICATION_LIMIT),
        page_size: Some(NOTIFICATION_PAGE_SIZE),
        ..Default::default()
    };

    let mut scan = NotificationScan::default();
    paginate_until(
        client,
        NOTIFICATIONS_QUERY,
        Map::new(),
        &["data", "notifications", "nodes"],
        &["data", "notifications", "pageInfo"],
        &options,
        NOTIFICATION_PAGE_SIZE,
        |nodes| scan.absorb(&nodes, pr_urls),
    )
    .await?;

    Ok(scan.entries)
}

/// Why `identifier` has no review URL, given the pull requests attached to it.
fn nothing_resolved_error(identifier: &str, pr_urls: &[String]) -> anyhow::Error {
    let reason = if pr_urls.is_empty() {
        "no pull request is linked to this issue".to_string()
    } else {
        format!(
            "Linear exposes a review URL through pull request notifications, and none of the {} \
             linked pull request(s) has one in the last {} notifications",
            pr_urls.len(),
            NOTIFICATION_LIMIT
        )
    };
    anyhow::anyhow!(
        "No review URL for {}: {}. Use the GitHub PR URL instead.",
        identifier,
        reason
    )
}

pub async fn show_review_url(issue_id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let result = client
        .query(ISSUE_QUERY, Some(json!({ "id": issue_id })))
        .await?;

    if result["data"]["issue"].is_null() {
        anyhow::bail!("Issue not found: {}", issue_id);
    }

    let issue: Issue = serde_json::from_value(result["data"]["issue"].clone())?;
    let url_key = result["data"]["organization"]["urlKey"]
        .as_str()
        .unwrap_or_default();

    let pr_urls = issue.attached_pr_urls();
    let notifications = if pr_urls.is_empty() {
        Vec::new()
    } else {
        notification_entries(&client, &pr_urls).await?
    };

    let resolution = Resolution::merge(
        &pr_urls,
        issue.agent_session_entries(url_key),
        notifications,
    );

    if resolution.resolved.is_empty() {
        let identifier = issue.identifier.as_deref().unwrap_or(issue_id);
        return Err(nothing_resolved_error(identifier, &pr_urls));
    }

    if output.is_json() || output.has_template() {
        return print_json(&json!(resolution), output);
    }

    for entry in &resolution.resolved {
        println!("{}", entry.review_url);
    }

    // A partly resolved issue still prints what it has, but never silently: the
    // pull requests it could not resolve are named on stderr so a caller reading
    // stdout does not mistake the list for the whole set.
    if !resolution.unresolved.is_empty() {
        eprintln!(
            "warning: no review URL for {} of the issue's pull request(s):",
            resolution.unresolved.len()
        );
        for url in &resolution.unresolved {
            eprintln!("  {}", url);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(value: Value) -> Issue {
        serde_json::from_value(value).expect("issue fixture must deserialize")
    }

    fn issue_with_sessions(sessions: Value) -> Issue {
        issue(json!({ "identifier": "LIN-123", "agentSessions": { "nodes": sessions } }))
    }

    fn pr_link(slug: &str, number: u64) -> Value {
        json!({ "pullRequest": {
            "slugId": slug,
            "number": number,
            "status": "open",
            "url": format!("https://github.com/acme/app/pull/{}", number),
            "title": "Fix the thing"
        }})
    }

    fn entry(review_url: &str, pr_url: &str) -> ReviewEntry {
        ReviewEntry {
            review_url: review_url.to_string(),
            number: None,
            status: None,
            title: None,
            url: pr_url.to_string(),
        }
    }

    #[test]
    fn test_attached_pr_urls_keeps_github_pull_requests_only() {
        let issue = issue(json!({ "attachments": { "nodes": [
            { "sourceType": "github", "url": "https://github.com/acme/app/pull/183" },
            { "sourceType": "github", "url": "https://github.com/acme/app/pull/183" },
            { "sourceType": "github", "url": "https://github.com/acme/app/issues/12" },
            { "sourceType": "sentry", "url": "https://sentry.io/acme/app/pull/1" },
            { "sourceType": "github", "url": "https://github.com/acme/app/pull/184" }
        ]}}));

        assert_eq!(
            issue.attached_pr_urls(),
            vec![
                "https://github.com/acme/app/pull/183",
                "https://github.com/acme/app/pull/184"
            ]
        );
    }

    #[test]
    fn test_review_entries_from_notifications_uses_the_notification_url() {
        let nodes = vec![
            json!({
                "__typename": "IssueNotification",
                "url": "https://linear.app/acme/issue/LIN-1"
            }),
            json!({
                "__typename": "PullRequestNotification",
                "url": "https://linear.app/acme/review/fix-the-thing-72e2bba2372a",
                "pullRequest": {
                    "url": "https://github.com/acme/app/pull/183",
                    "number": 183, "status": "open", "title": "Fix the thing"
                }
            }),
            json!({
                "__typename": "PullRequestNotification",
                "url": "https://linear.app/acme/review/someone-elses-pr-aaaaaaaaaaaa",
                "pullRequest": { "url": "https://github.com/acme/app/pull/999", "number": 999 }
            }),
        ];
        let wanted = vec!["https://github.com/acme/app/pull/183".to_string()];

        let entries = review_entries_from_notifications(&nodes, &wanted);

        assert_eq!(entries.len(), 1, "only the requested pull request matches");
        assert_eq!(
            entries[0].review_url, "https://linear.app/acme/review/fix-the-thing-72e2bba2372a",
            "the notification's URL is used verbatim, not reassembled from the slug"
        );
        assert_eq!(entries[0].number, Some(183));
    }

    #[test]
    fn test_review_entries_from_notifications_drops_a_comment_anchor() {
        let nodes = vec![json!({
            "__typename": "PullRequestNotification",
            "url": "https://linear.app/acme/review/fix-the-thing-72e2bba2372a#comment-5f63aa7c",
            "pullRequest": { "url": "https://github.com/acme/app/pull/183", "number": 183 }
        })];
        let wanted = vec!["https://github.com/acme/app/pull/183".to_string()];

        let entries = review_entries_from_notifications(&nodes, &wanted);

        assert_eq!(
            entries[0].review_url, "https://linear.app/acme/review/fix-the-thing-72e2bba2372a",
            "a comment notification must still yield the review page URL"
        );
    }

    #[test]
    fn test_review_entries_from_notifications_skips_a_pull_request_without_a_url() {
        // Nothing to key or report on, so it must not become an entry.
        let nodes = vec![json!({
            "__typename": "PullRequestNotification",
            "url": "https://linear.app/acme/review/fix-the-thing-72e2bba2372a",
            "pullRequest": { "number": 183 }
        })];
        let wanted = vec!["https://github.com/acme/app/pull/183".to_string()];

        assert!(review_entries_from_notifications(&nodes, &wanted).is_empty());
    }

    #[test]
    fn test_merge_prefers_the_notification_entry_per_pull_request() {
        let pr = "https://github.com/acme/app/pull/1";
        let other = "https://github.com/acme/app/pull/2";

        let merged = Resolution::merge(
            &[pr.to_string(), other.to_string()],
            vec![
                entry("from-agent-session", pr),
                entry("other-from-agent-session", other),
            ],
            vec![entry("from-notification", pr)],
        );

        assert_eq!(
            merged,
            Resolution {
                resolved: vec![
                    entry("from-notification", pr),
                    entry("other-from-agent-session", other)
                ],
                unresolved: vec![],
            }
        );
    }

    #[test]
    fn test_merge_reports_an_attached_pull_request_neither_source_resolved() {
        let resolved_pr = "https://github.com/acme/app/pull/1";
        let unresolved_pr = "https://github.com/acme/app/pull/2";

        let merged = Resolution::merge(
            &[resolved_pr.to_string(), unresolved_pr.to_string()],
            vec![],
            vec![entry("from-notification", resolved_pr)],
        );

        assert_eq!(
            merged,
            Resolution {
                resolved: vec![entry("from-notification", resolved_pr)],
                unresolved: vec![unresolved_pr.to_string()],
            },
            "a partial resolution must name the pull request it could not resolve"
        );
    }

    #[test]
    fn test_merge_reports_in_attachment_order_then_unattached_pull_requests() {
        let second = "https://github.com/acme/app/pull/99";
        let first = "https://github.com/acme/app/pull/183";
        let unattached = "https://github.com/acme/app/pull/7";

        let merged = Resolution::merge(
            // Attachment order, which is not the order the entries arrive in and
            // not lexicographic by URL.
            &[first.to_string(), second.to_string()],
            vec![entry("c", unattached)],
            vec![entry("b", second), entry("a", first)],
        );

        assert_eq!(
            merged.resolved,
            vec![
                entry("a", first),
                entry("b", second),
                entry("c", unattached)
            ]
        );
        assert!(merged.unresolved.is_empty());
    }

    #[test]
    fn test_agent_session_entries_builds_review_url_from_slug() {
        let issue = issue_with_sessions(json!([
            { "pullRequests": { "nodes": [pr_link("7ffd27854fd2", 183)] } }
        ]));

        let entries = issue.agent_session_entries("acme");

        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].review_url,
            "https://linear.app/acme/review/7ffd27854fd2"
        );
        assert_eq!(entries[0].number, Some(183));
        assert_eq!(entries[0].url, "https://github.com/acme/app/pull/183");
    }

    #[test]
    fn test_agent_session_entries_dedupe_a_pr_linked_by_several_sessions() {
        let issue = issue_with_sessions(json!([
            { "pullRequests": { "nodes": [pr_link("aaa111", 7)] } },
            { "pullRequests": { "nodes": [pr_link("aaa111", 7), pr_link("bbb222", 8)] } }
        ]));
        let pr_urls = vec![
            "https://github.com/acme/app/pull/7".to_string(),
            "https://github.com/acme/app/pull/8".to_string(),
        ];

        let merged = Resolution::merge(&pr_urls, issue.agent_session_entries("acme"), vec![]);

        assert_eq!(
            merged.resolved.len(),
            2,
            "the repeated pull request is listed once"
        );
        assert_eq!(
            merged.resolved[0].review_url,
            "https://linear.app/acme/review/aaa111"
        );
        assert_eq!(
            merged.resolved[1].review_url,
            "https://linear.app/acme/review/bbb222"
        );
    }

    #[test]
    fn test_agent_session_entries_empty_without_sessions_or_slug() {
        assert!(issue_with_sessions(json!([]))
            .agent_session_entries("acme")
            .is_empty());

        // A session with no linked pull request, and a link whose slug is missing
        // or blank: all unresolvable, and none of them may produce a bogus URL.
        let unresolvable = issue_with_sessions(json!([
            { "pullRequests": { "nodes": [] } },
            { "pullRequests": { "nodes": [{ "pullRequest": { "number": 1 } }] } },
            { "pullRequests": { "nodes": [{ "pullRequest": { "slugId": "", "number": 2 } }] } }
        ]));
        assert!(unresolvable.agent_session_entries("acme").is_empty());
    }

    #[test]
    fn test_resolution_serializes_both_halves() {
        let resolution = Resolution::merge(
            &[
                "https://github.com/acme/app/pull/1".to_string(),
                "https://github.com/acme/app/pull/2".to_string(),
            ],
            vec![],
            vec![ReviewEntry {
                review_url: "https://linear.app/acme/review/fix-the-thing-72e2bba2372a".to_string(),
                number: Some(1),
                status: Some("open".to_string()),
                title: Some("Fix the thing".to_string()),
                url: "https://github.com/acme/app/pull/1".to_string(),
            }],
        );

        assert_eq!(
            json!(resolution),
            json!({
                "resolved": [{
                    "reviewUrl": "https://linear.app/acme/review/fix-the-thing-72e2bba2372a",
                    "number": 1,
                    "status": "open",
                    "title": "Fix the thing",
                    "url": "https://github.com/acme/app/pull/1"
                }],
                "unresolved": ["https://github.com/acme/app/pull/2"]
            })
        );
    }

    #[test]
    fn test_nothing_resolved_error_distinguishes_no_pull_request_from_no_review_url() {
        assert!(nothing_resolved_error("LIN-123", &[])
            .to_string()
            .contains("no pull request is linked to this issue"));

        let attached = vec!["https://github.com/acme/app/pull/1".to_string()];
        let message = nothing_resolved_error("LIN-123", &attached).to_string();
        assert!(message.contains("1 linked pull request(s)"));
        assert!(message.contains(&NOTIFICATION_LIMIT.to_string()));
    }

    fn notification(review_url: &str, pr_url: &str) -> Value {
        json!({
            "__typename": "PullRequestNotification",
            "url": review_url,
            "pullRequest": { "url": pr_url }
        })
    }

    #[test]
    fn test_scan_stops_once_every_wanted_pull_request_is_found() {
        let first = "https://github.com/acme/app/pull/1";
        let second = "https://github.com/acme/app/pull/2";
        let wanted = vec![first.to_string(), second.to_string()];
        let mut scan = NotificationScan::default();

        assert_eq!(
            scan.absorb(&[notification("review-1", first)], &wanted),
            PageFlow::Continue,
            "one of two found: the rest of the feed is still worth reading"
        );
        assert_eq!(
            scan.absorb(&[notification("review-2", second)], &wanted),
            PageFlow::Stop,
            "both found: no further page may be requested"
        );
        assert_eq!(
            scan.entries,
            vec![entry("review-1", first), entry("review-2", second)]
        );
    }

    #[test]
    fn test_scan_keeps_the_newest_notification_per_pull_request() {
        let pr = "https://github.com/acme/app/pull/1";
        let other = "https://github.com/acme/app/pull/2";
        let wanted = vec![pr.to_string(), other.to_string()];
        let mut scan = NotificationScan::default();

        // The feed is newest-first, so a later page's older notification for the
        // same pull request must not displace the one already held.
        scan.absorb(&[notification("newest-review", pr)], &wanted);
        scan.absorb(&[notification("older-review", pr)], &wanted);

        assert_eq!(scan.entries, vec![entry("newest-review", pr)]);
    }

    #[test]
    fn test_scan_ignores_a_page_with_nothing_wanted_on_it() {
        let wanted = vec!["https://github.com/acme/app/pull/1".to_string()];
        let mut scan = NotificationScan::default();

        let flow = scan.absorb(
            &[notification(
                "review-x",
                "https://github.com/acme/app/pull/999",
            )],
            &wanted,
        );

        assert_eq!(flow, PageFlow::Continue);
        assert!(scan.entries.is_empty());
    }
}
