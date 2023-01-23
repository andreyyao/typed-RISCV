use annotate_snippets::display_list::{DisplayList, FormatOptions};
use annotate_snippets::snippet::{Annotation, Slice, Snippet};
use compiler::system_f::parse::parse_prog;
use compiler::system_f::semant::check_prog;

const PROGRAM: &str = "let all: Int -> Int -> (Int -> Bool) -> Bool =
  lambda min: Int. lambda max: Int. lambda pred: Int -> Bool.
    let folder: Int -> Bool -> Bool =
      lambda element: Int. lambda acc: Bool.
        if element - 1 < max
        then (pred element) && true
        else b + acc in
    folder min true";

fn main() {
    let prog = parse_prog(PROGRAM).unwrap();
    let result = check_prog(&prog);
    let err = result.unwrap_err();
    let snippet = Snippet {
        title: Some(Annotation {
            id: None,
            label: Some(err.title),
            annotation_type: err.annot_type,
        }),
        footer: vec![],
        slices: vec![Slice {
            source: PROGRAM,
            line_start: 1, // TODO
            origin: None,
            annotations: err.annotations,
            fold: false,
        }],
        opt: FormatOptions {
            color: true,
            anonymized_line_numbers: false,
            margin: None,
        },
    };
    let dlist: DisplayList = snippet.into();
    println!("{}", dlist)
}
