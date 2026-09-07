pub struct Row(Vec<String>);

impl Row {
    pub fn get(&self, index: usize) -> Option<&str> {
        self.0
            .get(index)
            .map(|field| field.trim())
            .filter(|field| !field.is_empty())
    }

    pub fn column(&self, index: usize) -> &str {
        self.get(index).unwrap_or_default()
    }
}

pub fn rows(table: &str, expected_header: &str) -> impl Iterator<Item = Row> {
    let mut lines = table.lines().filter(|line| !line.trim().is_empty());

    let found = lines.next().unwrap_or_default().trim_end();
    assert_eq!(
        found, expected_header,
        "a reference table's columns are not the ones its loader reads"
    );

    lines.map(|line| Row(fields(line)))
}

fn fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            c => field.push(c),
        }
    }

    fields.push(field);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "name,id,kind\nnasa,nasa,group\nnas a,nasa,\n\nesa,esa,group\n";

    #[test]
    fn the_header_is_not_a_row_and_neither_is_a_blank_line() {
        let rows: Vec<Vec<String>> = rows(SAMPLE, "name,id,kind")
            .map(|row| (0..3).map(|at| row.column(at).to_owned()).collect())
            .collect();

        assert_eq!(
            rows,
            [
                vec!["nasa", "nasa", "group"],
                vec!["nas a", "nasa", ""],
                vec!["esa", "esa", "group"],
            ]
        );
    }

    #[test]
    #[should_panic(expected = "not the ones its loader reads")]
    fn a_column_renamed_under_a_loader_stops_it_rather_than_shifting_the_data() {
        rows(SAMPLE, "name,id,sort").count();
    }

    #[test]
    fn a_field_may_hold_a_comma_when_it_is_quoted() {
        let row = fields(r#"Smith, Jr,"one, two",plain"#);
        assert_eq!(row, ["Smith", " Jr", "one, two", "plain"]);

        let escaped = fields(r#""a ""quoted"" name",x"#);
        assert_eq!(escaped, [r#"a "quoted" name"#, "x"]);
    }

    #[test]
    fn a_column_that_was_left_out_reads_as_nothing_rather_than_a_panic() {
        let row = rows("a,b\nonly,one\n", "a,b").next().unwrap();
        assert_eq!(row.get(0), Some("only"));
        assert_eq!(row.get(1), Some("one"));
        assert_eq!(row.get(2), None);
        assert_eq!(row.column(9), "");
    }
}
