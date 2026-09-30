use std::io::{self, Write};

pub(crate) fn selection_start(name: &str) -> usize {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let suffix = &stem[stem.trim_end_matches(|c: char| c.is_ascii_digit()).len()..];
    if !suffix.is_empty() && suffix.bytes().all(|digit| digit == b'0') {
        0
    } else {
        1
    }
}

pub(crate) fn parse_selection(input: &str, start: usize, end: usize) -> Option<Vec<usize>> {
    let mut indices = Vec::new();
    for token in input.split_whitespace() {
        let (first, last) = if let Some((left, right)) = token.split_once(':') {
            let first = if left.is_empty() {
                start
            } else {
                left.parse::<usize>().ok()?
            };
            let last = if right.is_empty() {
                end
            } else {
                right.parse::<usize>().ok()?
            };
            (first, last)
        } else {
            let number = token.parse::<usize>().ok()?;
            (number, number)
        };
        if first < start || last > end || first > last {
            return None;
        }
        indices.extend((first..=last).map(|number| number - start));
    }
    if indices.is_empty() {
        None
    } else {
        Some(indices)
    }
}

pub(crate) fn prompt(videos: &[String]) -> io::Result<Vec<usize>> {
    let first = videos
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No video files found"))?;
    let start = selection_start(first);
    let end = start + videos.len() - 1;

    for (i, name) in videos.iter().enumerate() {
        println!("{:>3}. {}", i + start, name);
    }

    print!("\nSelect videos (numbers or ranges like 2:, :4, 2:4; space-separated): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    parse_selection(&input, start, end).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Invalid selection: enter numbers or ascending ranges between {start} and {end} (e.g. 2:, :4, 2:4)"
            ),
        )
    })
}
