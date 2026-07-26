use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub fn truncate_to_width(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_owned();
    }
    let mut taken = String::new();
    let mut used = 0;
    for cluster in text.graphemes(true) {
        let cluster_width = display_width(cluster);
        if used + cluster_width > width {
            break;
        }
        taken.push_str(cluster);
        used += cluster_width;
    }
    taken
}

pub fn abbreviate_path(path: &str, width: usize) -> String {
    if display_width(path) <= width {
        return path.to_owned();
    }
    let Some((directories, name)) = path.rsplit_once('/') else {
        return shorten_name(path, width);
    };
    let parts: Vec<&str> = directories.split('/').collect();
    for abbreviated in 1..=parts.len() {
        let candidate = parts
            .iter()
            .enumerate()
            .map(|(index, part)| {
                if index < abbreviated {
                    first_cluster(part)
                } else {
                    (*part).to_owned()
                }
            })
            .chain(std::iter::once(name.to_owned()))
            .collect::<Vec<_>>()
            .join("/");
        if display_width(&candidate) <= width {
            return candidate;
        }
    }
    shorten_name(name, width)
}

fn first_cluster(part: &str) -> String {
    part.graphemes(true).next().unwrap_or_default().to_owned()
}

fn shorten_name(name: &str, width: usize) -> String {
    if display_width(name) <= width {
        return name.to_owned();
    }
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return truncate_to_width(name, width);
    };
    let tail = format!("….{extension}");
    let tail_width = display_width(&tail);
    if tail_width >= width {
        return truncate_to_width(name, width);
    }
    format!("{}{tail}", truncate_to_width(stem, width - tail_width))
}
