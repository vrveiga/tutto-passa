use anyhow::{Result, bail};

use crate::model::Data;

/// Exact id, else unique prefix, else an error with suggestions.
pub fn resolve(data: &Data, arg: &str) -> Result<usize> {
    if let Some(i) = data.find(arg) {
        return Ok(i);
    }
    let matches: Vec<usize> = (0..data.habits.len())
        .filter(|&i| data.habits[i].id.starts_with(arg))
        .collect();
    match matches[..] {
        [i] => Ok(i),
        [] => {
            let best = data
                .habits
                .iter()
                .map(|h| (levenshtein(arg, &h.id), &h.id))
                .filter(|(d, _)| *d <= 2)
                .min_by_key(|(d, _)| *d);
            match best {
                Some((_, id)) => bail!("no habit '{arg}' (did you mean {id}?)"),
                None => bail!("no habit '{arg}'"),
            }
        }
        _ => {
            let ids: Vec<&str> = matches
                .iter()
                .map(|&i| data.habits[i].id.as_str())
                .collect();
            bail!("'{arg}' matches {}", ids.join(", "))
        }
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::bool_habit;
    use jiff::civil::date;

    fn data(ids: &[&str]) -> Data {
        Data {
            habits: ids
                .iter()
                .map(|id| bool_habit(id, date(2026, 1, 1)))
                .collect(),
            ..Data::default()
        }
    }

    #[test]
    fn resolution() {
        let d = data(&["gym", "guitar", "go", "read"]);
        for (arg, want) in [
            ("go", Ok(2)),
            ("gy", Ok(0)),
            ("r", Ok(3)),
            ("g", Err("'g' matches gym, guitar, go")),
            ("gmy", Err("no habit 'gmy' (did you mean gym?)")),
            ("xyzzy", Err("no habit 'xyzzy'")),
        ] {
            let got = resolve(&d, arg).map_err(|e| e.to_string());
            assert_eq!(got, want.map_err(String::from), "{arg}");
        }
    }

    #[test]
    fn edit_distance() {
        assert_eq!(levenshtein("gmy", "gym"), 2);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("same", "same"), 0);
    }
}
