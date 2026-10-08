//! Grammaire des fences Mermaid partagée par les trois points d'entrée :
//! `StreamStateMachine` (filtre, pager, MCP detect), `PtyStreamProcessor`
//! (`strmaid run`) et `DiagramBlock::from_raw` (`--block-only`, MCP render).

use strmaid::domain::{DiagramBlock, GraphicsProtocol, ThemeMode, ViewportGeometry};
use strmaid::pty::PtyStreamProcessor;
use strmaid::renderer::RenderOptions;
use strmaid::stream::{StreamItem, StreamStateMachine};

struct Fixture {
    name: &'static str,
    input: &'static str,
    expected: Option<(&'static str, Option<&'static str>)>,
    single_block: bool,
}

const FIXTURES: [Fixture; 9] = [
    Fixture {
        name: "backticks avec titre",
        input: "```mermaid title=\"T\"\ngraph TD\n  A --> B\n```\n",
        expected: Some(("graph TD\n  A --> B", Some("T"))),
        single_block: true,
    },
    Fixture {
        name: "tildes",
        input: "~~~mermaid\ngraph TD\n  A --> B\n~~~\n",
        expected: Some(("graph TD\n  A --> B", None)),
        single_block: true,
    },
    Fixture {
        name: "fence longue contenant une ligne de trois backticks",
        input: "````mermaid\ngraph TD\n%% ```\n  A --> B\n````\n",
        expected: Some(("graph TD\n%% ```\n  A --> B", None)),
        single_block: true,
    },
    Fixture {
        name: "ligne ```js qui ne ferme pas le bloc",
        input: "```mermaid\ngraph TD\n```js\n  A --> B\n```\n",
        expected: Some(("graph TD\n```js\n  A --> B", None)),
        single_block: true,
    },
    Fixture {
        name: "langage mermaidjs",
        input: "```mermaidjs\ngraph TD\n  A --> B\n```\n",
        expected: Some(("graph TD\n  A --> B", None)),
        single_block: true,
    },
    Fixture {
        name: "bloc imbriqué dans une liste",
        input: "1. Étape\n   ```mermaid\n   graph TD\n     A --> B\n   ```\n",
        expected: Some(("   graph TD\n     A --> B", None)),
        single_block: false,
    },
    Fixture {
        name: "exemple mermaid dans un bloc markdown",
        input: "````markdown\n```mermaid\ngraph TD\n```\n````\n",
        expected: None,
        single_block: false,
    },
    Fixture {
        name: "langage voisin mermaid-x",
        input: "```mermaid-x\ngraph TD\n```\n",
        expected: None,
        single_block: false,
    },
    Fixture {
        name: "bloc rust",
        input: "```rust\nfn main() {}\n```\n",
        expected: None,
        single_block: false,
    },
];

fn stream_diagrams(input: &str) -> Vec<DiagramBlock> {
    let mut machine = StreamStateMachine::new();
    let mut items: Vec<StreamItem> = input
        .lines()
        .filter_map(|line| machine.process_line(line))
        .collect();
    items.extend(machine.finish());
    items
        .into_iter()
        .filter_map(|item| match item {
            StreamItem::Diagram(block) => Some(block),
            StreamItem::Text(_) | StreamItem::OversizedDiagram { .. } => None,
        })
        .collect()
}

fn pty_intercepts(input: &str) -> bool {
    let options = RenderOptions::new(
        ThemeMode::Mono,
        GraphicsProtocol::AsciiBox,
        ViewportGeometry::new(80, 24),
    );
    let mut output = Vec::new();
    let mut processor = PtyStreamProcessor::new(&mut output, options);
    let processed = processor
        .process_chunk(input)
        .and_then(|()| processor.finish());
    assert!(processed.is_ok());
    String::from_utf8_lossy(&output) != input.replace('\n', "\r\n")
}

#[test]
fn test_stream_recognizes_fixtures() {
    for fixture in &FIXTURES {
        let diagrams = stream_diagrams(fixture.input);
        let actual: Vec<(&str, Option<&str>)> = diagrams
            .iter()
            .map(|block| (block.as_str(), block.title()))
            .collect();
        let expected: Vec<(&str, Option<&str>)> = fixture.expected.into_iter().collect();
        assert_eq!(actual, expected, "stream : {}", fixture.name);
    }
}

#[test]
fn test_pty_intercepts_same_fixtures_as_stream() {
    for fixture in &FIXTURES {
        assert_eq!(
            pty_intercepts(fixture.input),
            fixture.expected.is_some(),
            "pty : {}",
            fixture.name
        );
    }
}

#[test]
fn test_from_raw_matches_stream_on_single_blocks() {
    for fixture in FIXTURES.iter().filter(|fixture| fixture.single_block) {
        let block = DiagramBlock::from_raw(fixture.input);
        assert_eq!(
            Some((block.as_str(), block.title())),
            fixture.expected,
            "from_raw : {}",
            fixture.name
        );
    }
}
