//! Typed structs for Linear API responses.
//!
//! These types provide structured access to Linear API data instead of working
//! with raw `serde_json::Value`. They are designed for gradual adoption - the
//! codebase can continue using `Value` for complex nested data while leveraging
//! these types for common operations.

use serde::{Deserialize, Serialize};

/// A Linear issue with all commonly used fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub estimate: Option<f64>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub branch_name: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub canceled_at: Option<String>,
    #[serde(default)]
    pub archived_at: Option<String>,
    #[serde(default)]
    pub state: Option<WorkflowState>,
    #[serde(default)]
    pub assignee: Option<User>,
    #[serde(default)]
    pub creator: Option<User>,
    #[serde(default)]
    pub team: Option<Team>,
    #[serde(default)]
    pub project: Option<Project>,
    #[serde(default)]
    pub cycle: Option<Cycle>,
    #[serde(default)]
    pub labels: Option<LabelConnection>,
    #[serde(default)]
    pub parent: Option<Box<IssueRef>>,
    #[serde(default)]
    pub sub_issues: Option<IssueConnection>,
}

/// A minimal issue reference for parent/child relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRef {
    pub id: String,
    pub identifier: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub state: Option<WorkflowState>,
}

/// Connection wrapper for issues (for sub-issues, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueConnection {
    pub nodes: Vec<IssueRef>,
}

/// A workflow state (issue status).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowState {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub state_type: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub position: Option<f64>,
}

/// A Linear user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub admin: Option<bool>,
}

/// A Linear team.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: String,
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub private: Option<bool>,
    #[serde(default)]
    pub issue_count: Option<i64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A project status reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    pub name: String,
}

/// A Linear project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub slug_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub status: Option<ProjectStatus>,
    #[serde(default)]
    pub progress: Option<f64>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub target_date: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub canceled_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub labels: Option<LabelConnection>,
}

/// A sprint cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    pub id: String,
    #[serde(default)]
    pub number: Option<i32>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub starts_at: Option<String>,
    #[serde(default)]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub progress: Option<f64>,
}

/// An issue label.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub parent: Option<Box<LabelRef>>,
}

/// A minimal label reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelRef {
    pub id: String,
    pub name: String,
}

/// Connection wrapper for labels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelConnection {
    pub nodes: Vec<Label>,
}

/// A comment on an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub edited_at: Option<String>,
}

/// A document in Linear.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub slug_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub creator: Option<User>,
    #[serde(default)]
    pub project: Option<Project>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A notification from Linear.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub notification_type: Option<String>,
    #[serde(default)]
    pub read_at: Option<String>,
    #[serde(default)]
    pub snoozed_until_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub issue: Option<IssueRef>,
    #[serde(default)]
    pub comment: Option<Comment>,
    #[serde(default)]
    pub actor: Option<User>,
}

/// Pagination info from GraphQL connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    #[serde(default)]
    pub has_next_page: bool,
    #[serde(default)]
    pub has_previous_page: bool,
    #[serde(default)]
    pub start_cursor: Option<String>,
    #[serde(default)]
    pub end_cursor: Option<String>,
}

/// A roadmap in Linear.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Roadmap {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub slug_id: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// An initiative in Linear.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Initiative {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub slug_id: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub sort_order: Option<f64>,
    #[serde(default)]
    pub target_date: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A favorite item in Linear.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub favorite_type: Option<String>,
    #[serde(default)]
    pub sort_order: Option<f64>,
    #[serde(default)]
    pub issue: Option<IssueRef>,
    #[serde(default)]
    pub project: Option<Project>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// An issue relation (blocking, related, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRelation {
    pub id: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub relation_type: Option<String>,
    #[serde(default)]
    pub issue: Option<IssueRef>,
    #[serde(default)]
    pub related_issue: Option<IssueRef>,
    #[serde(default)]
    pub created_at: Option<String>,
}

/// A time tracking entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: String,
    #[serde(default)]
    pub hours: Option<f64>,
    /// Duration in minutes (returned by timeSchedules GraphQL query).
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub issue: Option<IssueRef>,
    #[serde(default)]
    pub spent_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// The current viewer (authenticated user).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub admin: Option<bool>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Organization info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub url_key: Option<String>,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_deserialize() {
        let json = r#"{
            "id": "abc123",
            "identifier": "LIN-123",
            "title": "Test issue",
            "priority": 2,
            "state": {
                "id": "state1",
                "name": "In Progress",
                "type": "started"
            },
            "assignee": {
                "id": "user1",
                "name": "John Doe",
                "email": "john@example.com"
            },
            "team": {
                "id": "team1",
                "key": "ENG",
                "name": "Engineering"
            },
            "labels": {
                "nodes": [
                    {"id": "label1", "name": "bug"},
                    {"id": "label2", "name": "urgent"}
                ]
            }
        }"#;

        let issue: Issue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.identifier, "LIN-123");
        assert_eq!(issue.title, "Test issue");
        assert_eq!(issue.priority, 2);
        assert_eq!(issue.state.as_ref().unwrap().name, "In Progress");
        assert_eq!(issue.assignee.as_ref().unwrap().name, "John Doe");
        assert_eq!(issue.team.as_ref().unwrap().key, "ENG");
        assert_eq!(issue.labels.as_ref().unwrap().nodes.len(), 2);
    }

    #[test]
    fn test_issue_minimal() {
        let json = r#"{
            "id": "abc123",
            "identifier": "LIN-123",
            "title": "Test issue"
        }"#;

        let issue: Issue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.identifier, "LIN-123");
        assert!(issue.state.is_none());
        assert!(issue.assignee.is_none());
    }

    #[test]
    fn test_user_deserialize() {
        let json = r#"{
            "id": "user1",
            "name": "Jane Doe",
            "email": "jane@example.com",
            "active": true
        }"#;

        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.name, "Jane Doe");
        assert_eq!(user.email.as_deref(), Some("jane@example.com"));
        assert_eq!(user.active, Some(true));
    }

    #[test]
    fn test_team_deserialize() {
        let json = r#"{
            "id": "team1",
            "key": "ENG",
            "name": "Engineering",
            "private": false
        }"#;

        let team: Team = serde_json::from_str(json).unwrap();
        assert_eq!(team.key, "ENG");
        assert_eq!(team.name, "Engineering");
        assert_eq!(team.private, Some(false));
    }

    #[test]
    fn test_page_info_deserialize() {
        let json = r#"{
            "hasNextPage": true,
            "hasPreviousPage": false,
            "endCursor": "abc123"
        }"#;

        let page_info: PageInfo = serde_json::from_str(json).unwrap();
        assert!(page_info.has_next_page);
        assert!(!page_info.has_previous_page);
        assert_eq!(page_info.end_cursor.as_deref(), Some("abc123"));
    }

    // --- Cycle tests ---

    #[test]
    fn test_cycle_deserialize() {
        let json = r#"{
            "id": "c1",
            "number": 5,
            "name": "Sprint 5",
            "startsAt": "2024-01-01",
            "endsAt": "2024-01-14",
            "progress": 0.75
        }"#;
        let cycle: Cycle = serde_json::from_str(json).unwrap();
        assert_eq!(cycle.id, "c1");
        assert_eq!(cycle.number, Some(5));
        assert_eq!(cycle.name.as_deref(), Some("Sprint 5"));
        assert_eq!(cycle.starts_at.as_deref(), Some("2024-01-01"));
        assert_eq!(cycle.ends_at.as_deref(), Some("2024-01-14"));
        assert_eq!(cycle.progress, Some(0.75));
    }

    #[test]
    fn test_cycle_minimal() {
        let json = r#"{"id": "c1"}"#;
        let cycle: Cycle = serde_json::from_str(json).unwrap();
        assert_eq!(cycle.id, "c1");
        assert!(cycle.name.is_none());
        assert!(cycle.number.is_none());
        assert!(cycle.progress.is_none());
    }

    // --- Notification tests ---

    #[test]
    fn test_notification_deserialize() {
        let json = r#"{
            "id": "notif1",
            "type": "issueComment",
            "readAt": null,
            "createdAt": "2024-01-15T10:00:00Z",
            "issue": {
                "id": "issue1",
                "identifier": "LIN-100",
                "title": "Fix login bug"
            },
            "actor": {
                "id": "user1",
                "name": "Alice"
            }
        }"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.id, "notif1");
        assert_eq!(notif.notification_type.as_deref(), Some("issueComment"));
        assert!(notif.read_at.is_none());
        assert_eq!(notif.issue.as_ref().unwrap().identifier, "LIN-100");
        assert_eq!(notif.actor.as_ref().unwrap().name, "Alice");
    }

    #[test]
    fn test_notification_minimal() {
        let json = r#"{"id": "notif1"}"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.id, "notif1");
        assert!(notif.notification_type.is_none());
        assert!(notif.issue.is_none());
        assert!(notif.actor.is_none());
    }

    // --- IssueRelation tests ---

    #[test]
    fn test_issue_relation_deserialize() {
        let json = r#"{
            "id": "rel1",
            "type": "blocks",
            "issue": {
                "id": "issue1",
                "identifier": "LIN-1",
                "title": "Parent task"
            },
            "relatedIssue": {
                "id": "issue2",
                "identifier": "LIN-2",
                "title": "Blocked task"
            },
            "createdAt": "2024-02-01T12:00:00Z"
        }"#;
        let rel: IssueRelation = serde_json::from_str(json).unwrap();
        assert_eq!(rel.id, "rel1");
        assert_eq!(rel.relation_type.as_deref(), Some("blocks"));
        assert_eq!(rel.issue.as_ref().unwrap().identifier, "LIN-1");
        assert_eq!(rel.related_issue.as_ref().unwrap().identifier, "LIN-2");
        assert_eq!(rel.created_at.as_deref(), Some("2024-02-01T12:00:00Z"));
    }

    #[test]
    fn test_issue_relation_minimal() {
        let json = r#"{"id": "rel1"}"#;
        let rel: IssueRelation = serde_json::from_str(json).unwrap();
        assert_eq!(rel.id, "rel1");
        assert!(rel.relation_type.is_none());
        assert!(rel.issue.is_none());
        assert!(rel.related_issue.is_none());
    }

    // --- TimeEntry tests ---

    #[test]
    fn test_time_entry_deserialize() {
        let json = r#"{
            "id": "te1",
            "hours": 2.5,
            "spentAt": "2024-03-10",
            "user": {
                "id": "user1",
                "name": "Bob"
            },
            "issue": {
                "id": "issue1",
                "identifier": "LIN-50"
            },
            "createdAt": "2024-03-10T14:00:00Z"
        }"#;
        let entry: TimeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "te1");
        assert_eq!(entry.hours, Some(2.5));
        assert_eq!(entry.spent_at.as_deref(), Some("2024-03-10"));
        assert_eq!(entry.user.as_ref().unwrap().name, "Bob");
        assert_eq!(entry.issue.as_ref().unwrap().identifier, "LIN-50");
    }

    #[test]
    fn test_time_entry_minimal() {
        let json = r#"{"id": "te1"}"#;
        let entry: TimeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "te1");
        assert!(entry.hours.is_none());
        assert!(entry.user.is_none());
        assert!(entry.issue.is_none());
    }

    // --- Roadmap tests ---

    #[test]
    fn test_roadmap_deserialize() {
        let json = r#"{
            "id": "rm1",
            "name": "Q1 2024 Roadmap",
            "description": "First quarter planning",
            "slugId": "q1-2024",
            "createdAt": "2024-01-01T00:00:00Z"
        }"#;
        let roadmap: Roadmap = serde_json::from_str(json).unwrap();
        assert_eq!(roadmap.id, "rm1");
        assert_eq!(roadmap.name, "Q1 2024 Roadmap");
        assert_eq!(
            roadmap.description.as_deref(),
            Some("First quarter planning")
        );
        assert_eq!(roadmap.slug_id.as_deref(), Some("q1-2024"));
    }

    #[test]
    fn test_roadmap_minimal() {
        let json = r#"{"id": "rm1", "name": "Roadmap"}"#;
        let roadmap: Roadmap = serde_json::from_str(json).unwrap();
        assert_eq!(roadmap.id, "rm1");
        assert_eq!(roadmap.name, "Roadmap");
        assert!(roadmap.description.is_none());
        assert!(roadmap.slug_id.is_none());
    }

    // --- Initiative tests ---

    #[test]
    fn test_initiative_deserialize() {
        let json = r##"{
            "id": "init1",
            "name": "Platform Migration",
            "description": "Migrate to new platform",
            "slugId": "platform-migration",
            "color": "#FF5733",
            "icon": "rocket",
            "targetDate": "2024-06-30",
            "createdAt": "2024-01-15T10:00:00Z"
        }"##;
        let init: Initiative = serde_json::from_str(json).unwrap();
        assert_eq!(init.id, "init1");
        assert_eq!(init.name, "Platform Migration");
        assert_eq!(init.description.as_deref(), Some("Migrate to new platform"));
        assert_eq!(init.color.as_deref(), Some("#FF5733"));
        assert_eq!(init.target_date.as_deref(), Some("2024-06-30"));
    }

    #[test]
    fn test_initiative_minimal() {
        let json = r#"{"id": "init1", "name": "Init"}"#;
        let init: Initiative = serde_json::from_str(json).unwrap();
        assert_eq!(init.id, "init1");
        assert_eq!(init.name, "Init");
        assert!(init.description.is_none());
        assert!(init.target_date.is_none());
    }

    // --- Favorite tests ---

    #[test]
    fn test_favorite_deserialize() {
        let json = r#"{
            "id": "fav1",
            "type": "issue",
            "sortOrder": 1.5,
            "issue": {
                "id": "issue1",
                "identifier": "LIN-42",
                "title": "Favorite issue"
            },
            "createdAt": "2024-02-20T08:00:00Z"
        }"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.id, "fav1");
        assert_eq!(fav.favorite_type.as_deref(), Some("issue"));
        assert_eq!(fav.sort_order, Some(1.5));
        assert_eq!(fav.issue.as_ref().unwrap().identifier, "LIN-42");
    }

    #[test]
    fn test_favorite_minimal() {
        let json = r#"{"id": "fav1"}"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.id, "fav1");
        assert!(fav.favorite_type.is_none());
        assert!(fav.issue.is_none());
        assert!(fav.project.is_none());
    }

    // --- Document tests ---

    #[test]
    fn test_document_deserialize() {
        let json = r##"{
            "id": "doc1",
            "title": "Design Document",
            "content": "Overview of the design.",
            "slugId": "design-doc",
            "url": "https://linear.app/docs/design-doc",
            "icon": "file",
            "color": "#3B82F6",
            "creator": {
                "id": "user1",
                "name": "Alice"
            },
            "createdAt": "2024-03-01T09:00:00Z",
            "updatedAt": "2024-03-05T15:00:00Z"
        }"##;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.id, "doc1");
        assert_eq!(doc.title, "Design Document");
        assert!(doc.content.as_deref().unwrap().contains("Overview"));
        assert_eq!(doc.slug_id.as_deref(), Some("design-doc"));
        assert_eq!(doc.creator.as_ref().unwrap().name, "Alice");
    }

    #[test]
    fn test_document_minimal() {
        let json = r#"{"id": "doc1", "title": "Doc"}"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.id, "doc1");
        assert_eq!(doc.title, "Doc");
        assert!(doc.content.is_none());
        assert!(doc.creator.is_none());
    }

    // --- Label tests ---

    #[test]
    fn test_label_deserialize() {
        let json = r##"{
            "id": "label1",
            "name": "bug",
            "description": "Something is broken",
            "color": "#EF4444"
        }"##;
        let label: Label = serde_json::from_str(json).unwrap();
        assert_eq!(label.id, "label1");
        assert_eq!(label.name, "bug");
        assert_eq!(label.description.as_deref(), Some("Something is broken"));
        assert_eq!(label.color.as_deref(), Some("#EF4444"));
        assert!(label.parent.is_none());
    }

    #[test]
    fn test_label_with_parent() {
        let json = r#"{
            "id": "label2",
            "name": "frontend-bug",
            "parent": {
                "id": "label1",
                "name": "bug"
            }
        }"#;
        let label: Label = serde_json::from_str(json).unwrap();
        assert_eq!(label.id, "label2");
        assert_eq!(label.name, "frontend-bug");
        let parent = label.parent.as_ref().unwrap();
        assert_eq!(parent.id, "label1");
        assert_eq!(parent.name, "bug");
    }

    // --- Comment tests ---

    #[test]
    fn test_comment_deserialize() {
        let json = r#"{
            "id": "comment1",
            "body": "Looks good to me!",
            "user": {
                "id": "user1",
                "name": "Charlie"
            },
            "createdAt": "2024-04-01T11:30:00Z",
            "updatedAt": "2024-04-01T11:30:00Z",
            "editedAt": null
        }"#;
        let comment: Comment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.id, "comment1");
        assert_eq!(comment.body.as_deref(), Some("Looks good to me!"));
        assert_eq!(comment.user.as_ref().unwrap().name, "Charlie");
        assert!(comment.edited_at.is_none());
    }

    #[test]
    fn test_comment_minimal() {
        let json = r#"{"id": "comment1"}"#;
        let comment: Comment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.id, "comment1");
        assert!(comment.body.is_none());
        assert!(comment.user.is_none());
    }

    // --- WorkflowState tests ---

    #[test]
    fn test_workflow_state_deserialize() {
        let json = r##"{
            "id": "state1",
            "name": "In Progress",
            "type": "started",
            "color": "#F59E0B",
            "position": 2.0
        }"##;
        let state: WorkflowState = serde_json::from_str(json).unwrap();
        assert_eq!(state.id, "state1");
        assert_eq!(state.name, "In Progress");
        assert_eq!(state.state_type.as_deref(), Some("started"));
        assert_eq!(state.color.as_deref(), Some("#F59E0B"));
        assert_eq!(state.position, Some(2.0));
    }

    #[test]
    fn test_workflow_state_minimal() {
        let json = r#"{"id": "state1", "name": "Todo"}"#;
        let state: WorkflowState = serde_json::from_str(json).unwrap();
        assert_eq!(state.id, "state1");
        assert_eq!(state.name, "Todo");
        assert!(state.state_type.is_none());
        assert!(state.color.is_none());
    }

    // --- Viewer tests ---

    #[test]
    fn test_viewer_deserialize() {
        let json = r#"{
            "id": "viewer1",
            "name": "Current User",
            "email": "me@example.com",
            "displayName": "Me",
            "active": true,
            "admin": false,
            "url": "https://linear.app/user/me",
            "createdAt": "2023-01-01T00:00:00Z"
        }"#;
        let viewer: Viewer = serde_json::from_str(json).unwrap();
        assert_eq!(viewer.id, "viewer1");
        assert_eq!(viewer.name, "Current User");
        assert_eq!(viewer.email.as_deref(), Some("me@example.com"));
        assert_eq!(viewer.display_name.as_deref(), Some("Me"));
        assert_eq!(viewer.active, Some(true));
        assert_eq!(viewer.admin, Some(false));
    }

    #[test]
    fn test_viewer_minimal() {
        let json = r#"{"id": "viewer1", "name": "User"}"#;
        let viewer: Viewer = serde_json::from_str(json).unwrap();
        assert_eq!(viewer.id, "viewer1");
        assert_eq!(viewer.name, "User");
        assert!(viewer.email.is_none());
        assert!(viewer.admin.is_none());
    }

    // --- Organization tests ---

    #[test]
    fn test_organization_deserialize() {
        let json = r#"{
            "id": "org1",
            "name": "Acme Corp",
            "urlKey": "acme",
            "logoUrl": "https://example.com/logo.png",
            "createdAt": "2022-06-15T00:00:00Z"
        }"#;
        let org: Organization = serde_json::from_str(json).unwrap();
        assert_eq!(org.id, "org1");
        assert_eq!(org.name, "Acme Corp");
        assert_eq!(org.url_key.as_deref(), Some("acme"));
        assert_eq!(
            org.logo_url.as_deref(),
            Some("https://example.com/logo.png")
        );
    }

    #[test]
    fn test_organization_minimal() {
        let json = r#"{"id": "org1", "name": "Org"}"#;
        let org: Organization = serde_json::from_str(json).unwrap();
        assert_eq!(org.id, "org1");
        assert_eq!(org.name, "Org");
        assert!(org.url_key.is_none());
        assert!(org.logo_url.is_none());
    }
}
