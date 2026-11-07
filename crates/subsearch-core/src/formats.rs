//! Reading and writing graphs.
//!
//! | Format | Read | Write | Notes |
//! |---|---|---|---|
//! | [`Format::Lists`] | yes | yes | USRA's '-1'-terminated adjacency lists (upper triangle on output) |
//! | [`Format::EdgeList`] | yes | yes | one `u v` (or `u,v`) pair per line; `# vertices: N` keeps isolated vertices |
//! | [`Format::Graph6`] | yes | yes | nauty's compact dense format; read by networkx, SageMath, House of Graphs |
//! | [`Format::Sparse6`] | yes | yes | nauty's compact sparse format |
//! | [`Format::MatrixMarket`] | yes | yes | `coordinate pattern symmetric`; read by SciPy, MATLAB, Julia |
//! | [`Format::Dot`] | no | yes | Graphviz, optionally highlighting a vertex set |
//! | [`Format::GraphML`] | no | yes | Gephi, Cytoscape, yEd, networkx, igraph |

use std::fmt::Write as _;
use std::path::Path;

use crate::Error;
use crate::graph::{Graph, GraphBuilder};
use crate::legacy::Scanner;

/// A graph file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Lists,
    EdgeList,
    Graph6,
    Sparse6,
    MatrixMarket,
    Dot,
    GraphML,
}

impl Format {
    /// All formats, with their names.
    pub const ALL: [(Format, &'static str); 7] = [
        (Format::Lists, "lists"),
        (Format::EdgeList, "edgelist"),
        (Format::Graph6, "graph6"),
        (Format::Sparse6, "sparse6"),
        (Format::MatrixMarket, "mtx"),
        (Format::Dot, "dot"),
        (Format::GraphML, "graphml"),
    ];

    /// The format's name.
    pub fn name(self) -> &'static str {
        Format::ALL.iter().find(|(f, _)| *f == self).unwrap().1
    }

    /// Guesses the format from a file extension.
    pub fn from_path(path: &Path) -> Option<Format> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "txt" | "lists" | "adj" => Format::Lists,
            "edges" | "edgelist" | "el" | "csv" | "tsv" => Format::EdgeList,
            "g6" | "graph6" => Format::Graph6,
            "s6" | "sparse6" => Format::Sparse6,
            "mtx" | "mm" => Format::MatrixMarket,
            "dot" | "gv" => Format::Dot,
            "graphml" => Format::GraphML,
            _ => return None,
        })
    }

    /// Whether graphs can be read in this format.
    pub fn readable(self) -> bool {
        !matches!(self, Format::Dot | Format::GraphML)
    }
}

impl std::str::FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase();
        let alias = match s.as_str() {
            "legacy" | "adj" | "usra" => "lists",
            "edges" | "el" | "csv" => "edgelist",
            "g6" => "graph6",
            "s6" => "sparse6",
            "matrixmarket" | "mm" => "mtx",
            "gv" | "graphviz" => "dot",
            other => other,
        };
        Format::ALL
            .iter()
            .find(|(_, n)| *n == alias)
            .map(|(f, _)| *f)
            .ok_or_else(|| {
                let names: Vec<_> = Format::ALL.iter().map(|(_, n)| *n).collect();
                format!(
                    "unknown format '{s}' (expected one of: {})",
                    names.join(", ")
                )
            })
    }
}

/// Options for writing formats that can carry more than the graph.
#[derive(Clone, Debug, Default)]
pub struct WriteOptions<'a> {
    /// Vertices to highlight (DOT) or flag with an `in_subgraph` attribute (GraphML).
    pub highlight: &'a [usize],
}

/// Parses a graph from `text` in `format`.
pub fn parse(text: &str, format: Format) -> Result<Graph, Error> {
    match format {
        Format::Lists => parse_lists(text),
        Format::EdgeList => parse_edge_list(text),
        Format::Graph6 => parse_graph6(text.trim()),
        Format::Sparse6 => parse_sparse6(text.trim()),
        Format::MatrixMarket => parse_matrix_market(text),
        Format::Dot | Format::GraphML => Err(Error::Invalid(format!(
            "reading {} is not supported",
            format.name()
        ))),
    }
}

/// Serialises `g` in `format`.
pub fn to_string(g: &Graph, format: Format, options: &WriteOptions) -> String {
    match format {
        Format::Lists => write_lists(g),
        Format::EdgeList => write_edge_list(g),
        Format::Graph6 => write_graph6(g) + "\n",
        Format::Sparse6 => write_sparse6(g) + "\n",
        Format::MatrixMarket => write_matrix_market(g),
        Format::Dot => write_dot(g, options.highlight),
        Format::GraphML => write_graphml(g, options.highlight),
    }
}

/// Reads a graph from a file, using `format` or else guessing it from the extension.
pub fn read_file(path: &Path, format: Option<Format>) -> Result<Graph, Error> {
    let format = format.or_else(|| Format::from_path(path)).ok_or_else(|| {
        Error::Invalid(format!(
            "cannot tell the format of {} from its extension; specify it",
            path.display()
        ))
    })?;
    let text = std::fs::read_to_string(path)?;
    parse(&text, format)
}

/// Writes a graph to a file, using `format` or else guessing it from the extension.
pub fn write_file(
    g: &Graph,
    path: &Path,
    format: Option<Format>,
    options: &WriteOptions,
) -> Result<(), Error> {
    let format = format.or_else(|| Format::from_path(path)).ok_or_else(|| {
        Error::Invalid(format!(
            "cannot tell the format of {} from its extension; specify it",
            path.display()
        ))
    })?;
    std::fs::write(path, to_string(g, format, options))?;
    Ok(())
}

fn parse_error(line: usize, msg: impl Into<String>) -> Error {
    Error::Parse {
        line: Some(line),
        msg: msg.into(),
    }
}

// ---- USRA adjacency lists

fn parse_lists(text: &str) -> Result<Graph, Error> {
    let mut sc = Scanner::new(text);
    let mut rows: Vec<Vec<i64>> = vec![Vec::new()];
    while let Some(x) = sc.next_int() {
        if x == -1 {
            rows.push(Vec::new());
        } else {
            rows.last_mut().unwrap().push(x);
        }
    }
    if !rows.pop().unwrap().is_empty() {
        return Err(Error::Parse {
            line: None,
            msg: "the last adjacency list is not terminated by -1".into(),
        });
    }
    let n = rows.len();
    let mut b = GraphBuilder::new(n);
    for (i, row) in rows.iter().enumerate() {
        for &x in row {
            if x < 0 || x as usize >= n {
                return Err(parse_error(
                    i + 1,
                    format!("vertex {x} is out of range [0, {n})"),
                ));
            }
            if x as usize != i {
                b.set(i, x as usize, true);
            }
        }
    }
    Ok(b.build())
}

fn write_lists(g: &Graph) -> String {
    let mut s = String::new();
    for u in 0..g.num_vertices() {
        for &v in g.neighbors(u).iter().filter(|&&v| v as usize > u) {
            write!(s, " {v}").unwrap();
        }
        s.push_str("-1\n");
    }
    s
}

// ---- edge lists

fn parse_edge_list(text: &str) -> Result<Graph, Error> {
    let mut declared = None;
    let mut edges = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if let Some(comment) = line.strip_prefix('#').or_else(|| line.strip_prefix('%')) {
            if let Some(n) = comment.trim().strip_prefix("vertices:") {
                declared = Some(
                    n.trim()
                        .parse::<usize>()
                        .map_err(|_| parse_error(i + 1, "bad vertex count"))?,
                );
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let mut fields = line
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|f| !f.is_empty());
        let mut endpoint = || -> Result<usize, Error> {
            let f = fields
                .next()
                .ok_or_else(|| parse_error(i + 1, "expected two vertices"))?;
            f.parse()
                .map_err(|_| parse_error(i + 1, format!("'{f}' is not a vertex number")))
        };
        let (u, v) = (endpoint()?, endpoint()?);
        edges.push((u, v));
    }
    let n = declared.unwrap_or_else(|| edges.iter().map(|&(u, v)| u.max(v) + 1).max().unwrap_or(0));
    Graph::from_edges(n, edges)
}

fn write_edge_list(g: &Graph) -> String {
    let mut s = format!(
        "# vertices: {}\n# edges: {}\n",
        g.num_vertices(),
        g.num_edges()
    );
    for (u, v) in g.edges() {
        writeln!(s, "{u} {v}").unwrap();
    }
    s
}

// ---- graph6 and sparse6 (https://users.cecs.anu.edu.au/~bdm/data/formats.txt)

fn encode_n(n: usize, out: &mut Vec<u8>) {
    if n <= 62 {
        out.push(n as u8 + 63);
    } else if n <= 258_047 {
        out.push(126);
        for shift in [12, 6, 0] {
            out.push(((n >> shift) & 63) as u8 + 63);
        }
    } else {
        out.extend([126, 126]);
        for shift in [30, 24, 18, 12, 6, 0] {
            out.push(((n >> shift) & 63) as u8 + 63);
        }
    }
}

fn decode_n(bytes: &[u8]) -> Result<(usize, &[u8]), Error> {
    let bad = || Error::Parse {
        line: Some(1),
        msg: "truncated or invalid vertex count".into(),
    };
    let value = |chunk: &[u8]| {
        chunk.iter().try_fold(0usize, |acc, &c| {
            (63..=126)
                .contains(&c)
                .then(|| acc << 6 | (c - 63) as usize)
        })
    };
    match bytes {
        [126, 126, rest @ ..] if rest.len() >= 6 => {
            Ok((value(&rest[..6]).ok_or_else(bad)?, &rest[6..]))
        }
        [126, rest @ ..] if rest.len() >= 3 => Ok((value(&rest[..3]).ok_or_else(bad)?, &rest[3..])),
        [c, rest @ ..] if (63..=125).contains(c) => Ok(((c - 63) as usize, rest)),
        _ => Err(bad()),
    }
}

fn six_bit_groups(data: &[u8]) -> Result<Vec<u8>, Error> {
    data.iter()
        .map(|&c| {
            (63..=126)
                .contains(&c)
                .then(|| c - 63)
                .ok_or_else(|| Error::Parse {
                    line: Some(1),
                    msg: format!("invalid character {:?}", c as char),
                })
        })
        .collect()
}

fn pack_bits(bits: &[bool], out: &mut Vec<u8>) {
    for chunk in bits.chunks(6) {
        let mut x = 0u8;
        for (k, &b) in chunk.iter().enumerate() {
            x |= u8::from(b) << (5 - k);
        }
        out.push(x + 63);
    }
}

fn parse_graph6(text: &str) -> Result<Graph, Error> {
    let text = text.strip_prefix(">>graph6<<").unwrap_or(text);
    let line = text.lines().next().unwrap_or("");
    let (n, data) = decode_n(line.as_bytes())?;
    let groups = six_bit_groups(data)?;
    let needed = (n * n.saturating_sub(1) / 2).div_ceil(6);
    if groups.len() != needed {
        return Err(Error::Parse {
            line: Some(1),
            msg: format!(
                "expected {needed} data bytes for {n} vertices, got {}",
                groups.len()
            ),
        });
    }
    let mut b = GraphBuilder::new(n);
    let mut k = 0;
    for j in 1..n {
        for i in 0..j {
            if groups[k / 6] >> (5 - k % 6) & 1 == 1 {
                b.set(i, j, true);
            }
            k += 1;
        }
    }
    Ok(b.build())
}

/// The graph6 encoding of `g`, without header or newline.
pub fn write_graph6(g: &Graph) -> String {
    let n = g.num_vertices();
    let mut out = Vec::new();
    encode_n(n, &mut out);
    let mut bits = Vec::with_capacity(n * n.saturating_sub(1) / 2);
    for j in 1..n {
        for i in 0..j {
            bits.push(g.has_edge(i, j));
        }
    }
    pack_bits(&bits, &mut out);
    String::from_utf8(out).unwrap()
}

fn sparse6_k(n: usize) -> usize {
    let mut k = 1;
    while (1usize << k) < n {
        k += 1;
    }
    k
}

fn parse_sparse6(text: &str) -> Result<Graph, Error> {
    let text = text.strip_prefix(">>sparse6<<").unwrap_or(text);
    let line = text.lines().next().unwrap_or("");
    let body = line.strip_prefix(':').ok_or_else(|| Error::Parse {
        line: Some(1),
        msg: "sparse6 data must start with ':'".into(),
    })?;
    let (n, data) = decode_n(body.as_bytes())?;
    let groups = six_bit_groups(data)?;
    let k = sparse6_k(n);
    let total = groups.len() * 6;
    let bit = |p: usize| groups[p / 6] >> (5 - p % 6) & 1;
    let mut b = GraphBuilder::new(n);
    let (mut pos, mut v) = (0usize, 0usize);
    while pos + 1 + k <= total {
        let flag = bit(pos);
        let mut x = 0usize;
        for t in 0..k {
            x = x << 1 | bit(pos + 1 + t) as usize;
        }
        pos += 1 + k;
        if flag == 1 {
            v += 1;
        }
        if x >= n || v >= n {
            break;
        }
        if x > v {
            v = x;
        } else if x != v {
            b.set(x, v, true);
        }
    }
    Ok(b.build())
}

/// The sparse6 encoding of `g`, without header or newline.
pub fn write_sparse6(g: &Graph) -> String {
    let n = g.num_vertices();
    let k = sparse6_k(n);
    let mut bits = Vec::new();
    let push = |bits: &mut Vec<bool>, x: usize| {
        for t in (0..k).rev() {
            bits.push(x >> t & 1 == 1);
        }
    };
    let mut cur = 0usize;
    for v in 0..n {
        for &u in g.neighbors(v).iter().filter(|&&u| (u as usize) < v) {
            let u = u as usize;
            if v == cur {
                bits.push(false);
                push(&mut bits, u);
            } else if v == cur + 1 {
                cur += 1;
                bits.push(true);
                push(&mut bits, u);
            } else {
                cur = v;
                bits.push(true);
                push(&mut bits, v);
                bits.push(false);
                push(&mut bits, u);
            }
        }
    }
    let padding = (6 - bits.len() % 6) % 6;
    if k < 6 && n == (1 << k) && padding >= k && cur + 1 < n {
        bits.push(false);
    }
    let padding = (6 - bits.len() % 6) % 6;
    bits.extend(std::iter::repeat_n(true, padding));
    let mut out = vec![b':'];
    encode_n(n, &mut out);
    pack_bits(&bits, &mut out);
    String::from_utf8(out).unwrap()
}

// ---- Matrix Market

fn parse_matrix_market(text: &str) -> Result<Graph, Error> {
    let mut lines = text.lines().enumerate();
    let (_, header) = lines.next().ok_or_else(|| parse_error(1, "empty file"))?;
    let h: Vec<String> = header
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect();
    if h.len() < 5 || h[0] != "%%matrixmarket" || h[1] != "matrix" || h[2] != "coordinate" {
        return Err(parse_error(
            1,
            "expected a '%%MatrixMarket matrix coordinate ...' header",
        ));
    }
    let pattern = h[3] == "pattern";
    let mut size = None;
    let mut edges = Vec::new();
    for (i, line) in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('%') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |s: &str| {
            s.parse::<usize>()
                .map_err(|_| parse_error(i + 1, format!("'{s}' is not an index")))
        };
        match size {
            None => {
                if f.len() < 3 {
                    return Err(parse_error(i + 1, "expected 'rows columns entries'"));
                }
                let (r, c) = (num(f[0])?, num(f[1])?);
                if r != c {
                    return Err(parse_error(
                        i + 1,
                        format!("an adjacency matrix must be square, got {r} x {c}"),
                    ));
                }
                size = Some(r);
            }
            Some(n) => {
                if f.len() < 2 {
                    return Err(parse_error(i + 1, "expected 'row column [value]'"));
                }
                let (r, c) = (num(f[0])?, num(f[1])?);
                if r == 0 || c == 0 || r > n || c > n {
                    return Err(parse_error(i + 1, format!("index out of range [1, {n}]")));
                }
                let nonzero = pattern
                    || f.get(2)
                        .and_then(|v| v.parse::<f64>().ok())
                        .is_some_and(|v| v != 0.0);
                if nonzero && r != c {
                    edges.push((r - 1, c - 1));
                }
            }
        }
    }
    Graph::from_edges(
        size.ok_or_else(|| parse_error(1, "missing size line"))?,
        edges,
    )
}

fn write_matrix_market(g: &Graph) -> String {
    let n = g.num_vertices();
    let mut s = format!(
        "%%MatrixMarket matrix coordinate pattern symmetric\n{n} {n} {}\n",
        g.num_edges()
    );
    // symmetric storage keeps the lower triangle: row >= column
    for (u, v) in g.edges() {
        writeln!(s, "{} {}", v + 1, u + 1).unwrap();
    }
    s
}

// ---- Graphviz and GraphML

fn write_dot(g: &Graph, highlight: &[usize]) -> String {
    let n = g.num_vertices();
    let mut marked = vec![false; n];
    for &v in highlight.iter().filter(|&&v| v < n) {
        marked[v] = true;
    }
    let mut s = String::from("graph G {\n");
    if n > 20 {
        s.push_str("  layout=sfdp;\n  overlap=false;\n  splines=false;\n  node [shape=point, width=0.05, height=0.05];\n");
    } else {
        s.push_str("  layout=neato;\n  overlap=false;\n  node [shape=circle];\n");
    }
    if !highlight.is_empty() {
        s.push_str("  edge [color=gray70];\n");
        for v in (0..n).filter(|&v| marked[v]) {
            writeln!(s, "  {v} [color=red, style=filled, fillcolor=red];").unwrap();
        }
    }
    for (u, v) in g.edges() {
        if marked[u] && marked[v] {
            writeln!(s, "  {u} -- {v} [color=red, penwidth=2];").unwrap();
        } else {
            writeln!(s, "  {u} -- {v};").unwrap();
        }
    }
    for v in (0..n).filter(|&v| g.degree(v) == 0 && !marked[v]) {
        writeln!(s, "  {v};").unwrap();
    }
    s.push_str("}\n");
    s
}

fn write_graphml(g: &Graph, highlight: &[usize]) -> String {
    let n = g.num_vertices();
    let mut marked = vec![false; n];
    for &v in highlight.iter().filter(|&&v| v < n) {
        marked[v] = true;
    }
    let mut s = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\" ",
        "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
        "xsi:schemaLocation=\"http://graphml.graphdrawing.org/xmlns http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd\">\n",
    ));
    if !highlight.is_empty() {
        s.push_str("  <key id=\"in_subgraph\" for=\"node\" attr.name=\"in_subgraph\" attr.type=\"boolean\"><default>false</default></key>\n");
    }
    s.push_str("  <graph id=\"G\" edgedefault=\"undirected\">\n");
    for (v, &is_marked) in marked.iter().enumerate() {
        if is_marked {
            writeln!(
                s,
                "    <node id=\"n{v}\"><data key=\"in_subgraph\">true</data></node>"
            )
            .unwrap();
        } else {
            writeln!(s, "    <node id=\"n{v}\"/>").unwrap();
        }
    }
    for (u, v) in g.edges() {
        writeln!(s, "    <edge source=\"n{u}\" target=\"n{v}\"/>").unwrap();
    }
    s.push_str("  </graph>\n</graphml>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Graph {
        Graph::from_edges(9, [(0, 1), (0, 2), (1, 2), (5, 6), (7, 8), (2, 8)]).unwrap()
    }

    #[test]
    fn round_trips() {
        let g = sample();
        for format in [
            Format::Lists,
            Format::EdgeList,
            Format::Graph6,
            Format::Sparse6,
            Format::MatrixMarket,
        ] {
            let text = to_string(&g, format, &WriteOptions::default());
            assert_eq!(parse(&text, format).unwrap(), g, "{format:?}: {text}");
        }
        for n in [0, 1, 2, 62, 63, 64, 100, 300] {
            let g = Graph::from_edges(n, (1..n).step_by(3).map(|i| (i - 1, i))).unwrap();
            assert_eq!(parse_graph6(&write_graph6(&g)).unwrap(), g, "graph6 n={n}");
            assert_eq!(
                parse_sparse6(&write_sparse6(&g)).unwrap(),
                g,
                "sparse6 n={n}"
            );
        }
    }

    #[test]
    fn known_encodings() {
        // examples from nauty's formats.txt
        let g = Graph::from_edges(5, [(0, 2), (0, 4), (1, 3), (3, 4)]).unwrap();
        assert_eq!(write_graph6(&g), "DQc");
        let s = Graph::from_edges(7, [(0, 1), (0, 2), (1, 2), (5, 6)]).unwrap();
        assert_eq!(write_sparse6(&s), ":Fa@x^");
        assert_eq!(parse_sparse6(":Fa@x^").unwrap(), s);
    }

    #[test]
    fn edge_lists_keep_isolated_vertices() {
        let g = parse_edge_list("# vertices: 5\n0,1\n3 4\n").unwrap();
        assert_eq!((g.num_vertices(), g.num_edges()), (5, 2));
        assert_eq!(parse_edge_list("1 2\n").unwrap().num_vertices(), 3);
    }

    #[test]
    fn matrix_market_accepts_weighted_general_matrices() {
        let text = "%%MatrixMarket matrix coordinate real general\n% a comment\n3 3 4\n1 2 1.5\n2 1 1.5\n3 3 2.0\n1 3 0\n";
        let g = parse_matrix_market(text).unwrap();
        assert_eq!(g.edges().collect::<Vec<_>>(), vec![(0, 1)]);
    }
}
