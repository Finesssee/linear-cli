use std::io::{self, BufRead};

pub fn read_ids_from_stdin(ids: Vec<String>) -> Vec<String> {
    if ids.is_empty() || (ids.len() == 1 && ids[0] == "-") {
        let stdin = io::stdin();
        return stdin
            .lock()
            .lines()
            .map_while(Result::ok)
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
    }

    ids
}
