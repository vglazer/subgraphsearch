import json
import re
from pathlib import Path

import matplotlib
import numpy as np
import pytest

import subgraphsearch as ss
from subgraphsearch import convert, viz

matplotlib.use("Agg")

FIXTURES = Path(__file__).resolve().parents[2] / "tests" / "fixtures"


@pytest.fixture(scope="module")
def g100():
    return ss.generate("exponential", 100, 0.6, seed=1, rng="legacy")


def cycle(n):
    return ss.Graph(n, [(i, (i + 1) % n) for i in range(n)])


# ---- graphs

def test_construction_and_properties():
    g = ss.Graph(5, [(0, 1), (1, 2), (3, 4), (2, 1)])
    assert (g.n, g.m) == (5, 3)
    assert g.edges().tolist() == [[0, 1], [1, 2], [3, 4]]
    assert g.degrees().tolist() == [1, 2, 1, 1, 1]
    assert g.has_edge(2, 1) and not g.has_edge(0, 2)
    assert g.components()[1] == 2
    assert ss.Graph(5, np.array([[0, 1], [1, 2], [3, 4]])) == g
    with pytest.raises(ValueError, match="out of range"):
        ss.Graph(3, [(0, 3)])
    with pytest.raises(ValueError, match="self-loop"):
        ss.Graph(3, [(1, 1)])


def test_operations():
    g = cycle(6)
    assert g.complement().m == 15 - 6
    assert g.induced([0, 1, 3]).edge_list() == [(0, 1)]
    assert g.common_neighborhood([0, 2]) == [1]
    assert g.common_non_neighborhood([0]) == [2, 3, 4]
    s = g.switch([0])
    assert not s.has_edge(0, 1) and s.has_edge(0, 3)
    assert np.allclose(np.linalg.eigvalsh(g.laplacian()).sum(), 2 * g.m)


@pytest.mark.parametrize("fmt,ext", [("graph6", "g6"), ("sparse6", "s6"), ("mtx", "mtx"), ("edgelist", "edges"), ("lists", "txt")])
def test_format_round_trips(tmp_path, g100, fmt, ext):
    path = tmp_path / f"g.{ext}"
    g100.write(path)
    assert ss.read(path) == g100
    assert ss.Graph.parse(g100.dumps(fmt), fmt) == g100


def test_known_graph6_and_write_only_formats(tmp_path):
    g = ss.Graph.parse("DQc", "graph6")
    assert g.edge_list() == [(0, 2), (0, 4), (1, 3), (3, 4)]
    assert g.to_graph6() == "DQc"
    dot = g.dumps("dot", highlight=[0, 2])
    assert "0 -- 2 [color=red" in dot
    g.write(tmp_path / "g.graphml", highlight=[1])
    assert "in_subgraph" in (tmp_path / "g.graphml").read_text()
    assert ss.formats()["dot"] is False


def test_legacy_graphs_match_ggen():
    lists = (FIXTURES / "graphs" / "exp_100_600_1.txt").read_text()
    assert ss.Graph.parse(lists, "lists") == ss.generate("exponential", 100, 0.6, seed=1, rng="legacy")
    assert ss.generate("power", 300, 0.2, seed=52, rng="legacy") == ss.read(FIXTURES / "graphs" / "pow_300_200_52.txt")
    assert ss.generate("geometric", 400, 0.15, seed=1, rng="legacy") == ss.read(FIXTURES / "graphs" / "geo_400_150_1.txt")
    with pytest.raises(ValueError, match="steps of 0.001"):
        ss.generate("exponential", 10, 0.0005)


# ---- interop

def test_networkx_round_trip(g100):
    nx = pytest.importorskip("networkx")
    G = convert.to_networkx(g100)
    assert G.number_of_edges() == g100.m
    assert convert.from_networkx(G) == g100
    H = nx.Graph([("a", "b"), ("b", "c")])
    h, nodes = convert.from_networkx(H, return_nodes=True)
    assert nodes == ["a", "b", "c"] and h.m == 2


def test_igraph_scipy_numpy_round_trips(g100):
    pytest.importorskip("igraph")
    pytest.importorskip("scipy")
    assert convert.from_igraph(convert.to_igraph(g100)) == g100
    a = convert.to_scipy(g100)
    assert a.shape == (100, 100) and a.nnz == 2 * g100.m
    assert convert.from_scipy(a) == g100
    assert convert.from_numpy(convert.to_numpy(g100)) == g100


# ---- search

def test_search_finds_valid_subgraphs(g100):
    r = ss.search(g100, 8, 0, experiments=20, max_moves=300)
    assert r.successes == 20
    for e in r.experiments:
        assert len(e.vertices) == 8 and g100.induced_edge_count(e.vertices) == 0
    assert r.boxes().sum() == 20 and r.average_conflict == 0
    assert r.solutions() and len(set(r.solutions())) == len(r.solutions())


def test_search_is_deterministic_and_respects_fixed_vertices(g100):
    a = ss.search(g100, 9, 20, fixed=[3, 50], experiments=12, seed=7, trace=True)
    b = ss.search(g100, 9, 20, fixed=[3, 50], experiments=12, seed=7, trace=True)
    assert a.experiments == b.experiments
    for e in a.experiments:
        assert {3, 50} <= set(e.vertices)
        assert len(e.trace) == e.moves + e.success
        if e.success:
            assert g100.induced_edge_count(e.vertices) == 20 and e.trace[-1] == 0
    assert ss.search(g100, 9, 20, experiments=12, seed=8).experiments != a.experiments


def test_search_validation(g100):
    with pytest.raises(ValueError, match="not a valid subgraph size"):
        ss.search(g100, 100, 5)
    with pytest.raises(ValueError, match="diversification frequency"):
        ss.search(g100, 5, 5, div_freq=0)
    with pytest.raises(ValueError, match="listed twice"):
        ss.search(g100, 5, 5, fixed=[1, 1])


def test_legacy_search_matches_the_c_program_fixtures():
    strip = lambda s: re.sub(r"T> .*", "T>", s)  # noqa: E731
    for name in ["readme_cliques", "fixed_cliques", "approx_adj", "complement"]:
        out, status = ss.legacy.sub_search((FIXTURES / "sub_search" / f"{name}.in").read_text())
        assert strip(out) == strip((FIXTURES / "sub_search" / f"{name}.out").read_text()), name
        assert status == 0


def test_legacy_asub_search_matches_the_c_program_fixtures():
    strip = lambda s: re.sub(r"T> .*", "T>", s)  # noqa: E731
    for name in ["readme_cliques", "one_step_show", "approx_adj", "geometric"]:
        out, status = ss.legacy.asub_search((FIXTURES / "asub_search" / f"{name}.in").read_text())
        assert strip(out) == strip((FIXTURES / "asub_search" / f"{name}.out").read_text()), name
        assert status == 0


def test_aspiration_search(g100):
    plain = ss.search(g100, 11, 55, experiments=5, max_moves=100, div_freq=25, div_dur=4, seed=72, rng="legacy")
    asp = ss.search(g100, 11, 55, experiments=5, max_moves=100, div_freq=25, div_dur=4, seed=72, rng="legacy", aspiration=True)
    assert asp.params.aspiration and not plain.params.aspiration
    # the same as the C programs (see the readme_cliques fixtures): 5 and 4 successes
    assert (plain.successes, asp.successes) == (5, 4)
    r = ss.search(g100, 30, 340, experiments=20, max_moves=2000, div_freq=300, div_dur=8, aspiration=True)
    for e in r.experiments:
        if e.success:
            assert g100.induced_edge_count(e.vertices) == 340
    assert ss.SearchResult.from_dict(r.to_dict(), g100).params.aspiration


def test_legacy_ggen_and_errors():
    assert ss.legacy.ggen((FIXTURES / "ggen" / "exp_10_500_42.in").read_text()) == (FIXTURES / "ggen" / "exp_10_500_42.out").read_text()
    with pytest.raises(ValueError, match="seed must be a positive integer"):
        ss.legacy.ggen("2 6 0 500 0 0 0 0")


def test_result_serialization(g100):
    r = ss.search(g100, 6, 3, experiments=3, trace=True)
    d = json.loads(r.to_json())
    assert d["params"]["size"] == 6 and len(d["experiments"]) == 3
    assert isinstance(d["experiments"][0]["trace"], list)


def test_sweep(g100):
    res = ss.sweep(g100, 8, 0, "div_dur", [0, 4], experiments=4, max_moves=200)
    assert set(res) == {0, 4} and all(r.params.div_dur == v for v, r in res.items())


# ---- visualization

def test_plots_render(g100):
    r = ss.search(g100, 10, 30, experiments=8, max_moves=400, trace=True)
    for fn in (viz.plot_traces, viz.plot_run_time_distribution, viz.plot_success_times, viz.plot_conflicts):
        assert fn(r) is not None
    viz.plot_search_summary(r)
    viz.plot_sweep(ss.sweep(g100, 8, 0, "max_moves", [20, 200], experiments=4), parameter="max_moves")
    best = r.best.vertices
    for fn in (viz.plot_degree_distribution, viz.plot_spectrum):
        assert fn(g100) is not None
    for order in ("degree", "spectral", "index"):
        viz.plot_adjacency(g100, best, order=order)
    viz.plot_graph_summary(g100, best)
    for layout in ("spring", "circle", "spectral"):
        viz.plot_graph(g100, best, layout=layout, title="x")
    viz.plot_graph(ss.WeightedGraph.from_graph(cycle(10)), [0, 1, 2])
    ax = viz.plot_graph(cycle(6), [0, 1], labels={0: "a", 1: "b"})
    assert [t.get_text() for t in ax.texts] == ["a", "b"]
    with pytest.raises(ValueError, match="layout must have shape"):
        viz.plot_graph(g100, layout=np.zeros((3, 2)))
    import matplotlib.pyplot as plt

    plt.close("all")
    with pytest.raises(ValueError, match="traces"):
        viz.plot_traces(ss.search(g100, 5, 2, experiments=2))


def test_spectral_summary_and_sparse_spectrum():
    pytest.importorskip("scipy")
    two = ss.Graph(8, [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4)])
    s = viz.spectral_summary(two)
    assert s["components"] == 2 and s["algebraic_connectivity"] == 0.0
    assert np.isclose(s["trace"], 2 * two.m)
    c = cycle(50)
    dense = viz.laplacian_spectrum(c)
    assert np.allclose(viz.laplacian_spectrum(c, k=4), dense[:4], atol=1e-8)


def test_interactive_html(tmp_path, g100):
    r = ss.search(g100, 11, 55, experiments=5, max_moves=100, div_freq=25, div_dur=4, seed=72, rng="legacy")
    page = viz.to_html(r)
    data = json.loads(re.search(r"const DATA = (\{.*?\});\n", page).group(1))
    assert data["n"] == 100 and len(data["edges"]) == 2 * g100.m and len(data["pos"]) == 200
    assert len(data["solutions"]) == 5 and data["solutions"][0]["vertices"] == list(r.best.vertices)
    path = viz.save_html(tmp_path / "g.html", g100, highlight=[1, 2, 3], title="</script> test")
    text = Path(path).read_text()
    assert "&lt;/script&gt; test" in text and text.count("</script>") == 1
    pytest.importorskip("IPython")
    assert "srcdoc" in viz.show(g100).data


def test_layouts(g100):
    for method in ("spring", "spectral", "circle"):
        p = g100.layout(method)
        assert p.shape == (100, 2) and np.all((p >= -1e-9) & (p <= 1 + 1e-9))


def test_jupyter_reprs(g100):
    small = cycle(12)._repr_html_()
    # charts are img data URIs, so sanitizers in untrusted notebooks cannot strip them into stray text
    assert "data:image/svg+xml;base64," in small and "<svg" not in small and "<title" not in small
    assert "degree distribution" in g100._repr_html_()
    r = ss.search(g100, 8, 0, experiments=6, max_moves=200)
    h = r._repr_html_()
    assert "Solutions" in h and "moves to success" in h


# ---- CLI round trip: Rust search --json -> Python visualization

def test_rust_cli_json_loads_in_python(tmp_path, g100):
    import subprocess

    from subgraphsearch.__main__ import main

    exe = Path(__file__).resolve().parents[2] / "target" / "release" / "subgraphsearch"
    if not exe.exists():
        pytest.skip("build the CLI first: cargo build --release")
    graph_path = tmp_path / "g.g6"
    g100.write(graph_path)
    out = tmp_path / "r.json"
    subprocess.run([exe, "search", graph_path, "-k", "8", "-m", "0", "-n", "6", "--trace", "--json", out, "-q"], check=True)
    r = ss.SearchResult.load(out, g100)
    assert r.params.trace and r.successes == 6 and r.experiments[0].trace is not None
    py = ss.search(g100, 8, 0, experiments=6, trace=True)
    assert [e.vertices for e in r.experiments] == [e.vertices for e in py.experiments]
    r.save(tmp_path / "py.json")
    assert ss.SearchResult.load(tmp_path / "py.json", g100).experiments == r.experiments
    assert main(["explore", str(graph_path), "--result", str(out), "-o", str(tmp_path / "x.html")]) == 0
    assert main(["report", str(graph_path), "--result", str(out), "-o", str(tmp_path / "x.png")]) == 0
    assert (tmp_path / "x-search.png").exists() and (tmp_path / "x-graph.png").exists()


# ---- multi-graph files

def test_multi_graph_files(tmp_path):
    graphs = [ss.generate("exponential", 30, 0.4, seed=s) for s in (1, 2, 3)] + [cycle(5)]
    for ext in ("g6", "s6"):
        path = tmp_path / f"many.{ext}"
        ss.write_all(path, graphs)
        assert ss.read_all(path) == graphs
        assert ss.read(path, index=1) == graphs[1]
        assert ss.read(path, index=-1) == graphs[-1]
        with pytest.raises(ValueError, match="holds 4 graphs"):
            ss.read(path)
        with pytest.raises(IndexError, match="holds 4 graph"):
            ss.read(path, index=4)
    text = ss.dumps_all(graphs)
    assert len(text.splitlines()) == 4 and ss.parse_all(text, "graph6") == graphs
    assert ss.Graph.parse(text, "graph6", index=3) == graphs[3]
    assert ss.read_all(FIXTURES / "graphs" / "exp_30_300_5.txt") == [ss.read(FIXTURES / "graphs" / "exp_30_300_5.txt")]
    with pytest.raises(ValueError, match="single graph"):
        ss.dumps_all(graphs, "edgelist")
    with pytest.raises(ValueError, match="multi-graph format"):
        ss.write_all(tmp_path / "x.edges", graphs)


def test_graph6_and_sparse6_agree_with_networkx(tmp_path):
    nx = pytest.importorskip("networkx")
    rng = np.random.default_rng(0)
    graphs = [ss.Graph(0, [])]
    for n in [1, 2, 7, 62, 63, 64, 65, 100, 260]:
        for p in (0.02, 0.3):
            pairs = np.argwhere(np.triu(rng.random((n, n)) < p, k=1))
            graphs.append(ss.Graph(n, pairs))
    for fmt, to_bytes, reader in [("graph6", nx.to_graph6_bytes, nx.read_graph6), ("sparse6", nx.to_sparse6_bytes, nx.read_sparse6)]:
        # networkx writes, subgraphsearch reads
        nx_text = b"".join(to_bytes(convert.to_networkx(g), header=False) for g in graphs).decode()
        assert ss.parse_all(nx_text, fmt) == graphs, fmt
        # subgraphsearch writes, networkx reads
        path = tmp_path / f"many.{fmt}"
        ss.write_all(path, graphs, fmt)
        back = reader(path)
        assert [convert.from_networkx(h) for h in back] == graphs, fmt


# ---- weighted graphs (wsub_search, wggen)

@pytest.fixture(scope="module")
def w200():
    return ss.generate_weighted(200, 0.65, seed=1, rng="legacy")


def test_weighted_graph_basics():
    w = ss.WeightedGraph(4, [(0, 1, 3), (2, 1, 5)])
    assert (w.n, w.m, w.total_weight) == (4, 2, 8)
    assert w.weight(1, 0) == 3 and w.weight(0, 3) == 0
    assert w.edges().tolist() == [[0, 1, 3], [1, 2, 5]]
    assert w.weighted_degrees().tolist() == [3, 8, 5, 0]
    assert w.induced_weight([0, 1, 2]) == 8 and w.structure.m == 2
    assert ss.WeightedGraph.from_graph(w.structure).total_weight == 2
    with pytest.raises(ValueError, match="between 1 and 255"):
        ss.WeightedGraph(3, [(0, 1, 0)])


def test_generate_weighted_matches_wggen(w200):
    assert w200 == ss.WeightedGraph.read(FIXTURES / "graphs" / "w200_65_1.txt", "wlists")
    assert w200.total_weight == 12935
    assert ss.generate_weighted(30, 3.0, seed=5).total_weight == 30 * 29 * 300 // 200
    with pytest.raises(ValueError, match="multiple of 0.01"):
        ss.generate_weighted(10, 0.005)


@pytest.mark.parametrize("fmt,ext", [("edgelist", "edges"), ("mtx", "mtx"), ("wlists", "wl")])
def test_weighted_round_trips(tmp_path, w200, fmt, ext):
    path = tmp_path / f"w.{ext}"
    w200.write(path, fmt)
    assert ss.WeightedGraph.read(path, fmt) == w200
    with pytest.raises(ValueError, match="cannot store edge weights"):
        w200.dumps("graph6")


def test_weighted_interop(w200):
    nx = pytest.importorskip("networkx")
    pytest.importorskip("scipy")
    G = convert.to_networkx(w200)
    assert G.size(weight="weight") == w200.total_weight
    assert convert.from_networkx(G, weight="weight") == w200
    assert convert.from_networkx(G) == w200.structure
    a = convert.to_scipy(w200)
    assert a.sum() == 2 * w200.total_weight
    assert convert.from_scipy(a, weighted=True) == w200


def test_weighted_search(w200):
    r = ss.search(w200, 15, 180, experiments=30, max_moves=3000, div_freq=300, div_dur=10, seed=2)
    assert r.weighted and "total weight 180" in r.summary()
    assert r.successes > 0
    for e in r.experiments:
        if e.success:
            assert w200.induced_weight(e.vertices) == 180
    # legacy rng reproduces the C wsub_search fixture
    legacy = ss.search(w200, 10, 30, experiments=20, max_moves=1000, div_freq=200, div_dur=5, seed=3, rng="legacy")
    expected = (FIXTURES / "wsub_search" / "light.out").read_text()
    for e in legacy.experiments:
        assert " ".join(map(str, e.vertices)) + " " in expected


def test_legacy_weighted_programs():
    strip = lambda s: re.sub(r"T> .*", "T>", s)  # noqa: E731
    for name in ["light_adj", "multi_weights", "hard_183", "fixed", "unit_weights"]:
        out, status = ss.legacy.wsub_search((FIXTURES / "wsub_search" / f"{name}.in").read_text())
        assert strip(out) == strip((FIXTURES / "wsub_search" / f"{name}.out").read_text()), name
    for name in ["w10_50_3", "w60_150_3"]:
        args = (FIXTURES / "wggen" / f"{name}.args").read_text()
        assert ss.legacy.wggen(args) == (FIXTURES / "wggen" / f"{name}.out").read_text()
    with pytest.raises(ValueError, match="edge weight exceeds 127"):
        ss.legacy.wggen([10, 30000, 1])


def test_weighted_visualization(tmp_path, w200):
    r = ss.search(w200, 15, 180, experiments=6, max_moves=2000, seed=4, trace=True)
    page = viz.to_html(r)
    data = json.loads(re.search(r"const DATA = (\{.*?\});\n", page).group(1))
    assert len(data["weights"]) == w200.m and sum(data["weights"]) == w200.total_weight
    assert "total weight" in r._repr_html_() and "edge weights" in w200._repr_html_()
    viz.plot_search_summary(r)
    viz.plot_graph_summary(w200, r.best.vertices)
    import matplotlib.pyplot as plt

    plt.close("all")


# ---- regular subgraphs (reg_search, ereg_search)

def paley(q):
    squares = {x * x % q for x in range(1, q)}
    return ss.Graph(q, [(i, j) for i in range(q) for j in range(i + 1, q) if (j - i) % q in squares])


def test_regular_search():
    g = paley(61)
    assert g == ss.read(FIXTURES / "graphs" / "paley_61.txt")
    for two_phase in (False, True):
        r = ss.search(g, 10, degree=4, two_phase=two_phase, experiments=10, max_moves=2000, div_freq=300, div_dur=8, trace=True)
        assert r.target == "regular of degree 4" and "regular of degree 4" in r.summary()
        assert r.successes > 0
        for e in r.experiments:
            if e.success:
                sub = g.induced(e.vertices)
                assert set(sub.degrees().tolist()) == {4}
        back = ss.SearchResult.from_dict(json.loads(json.dumps(r.to_dict())), g)
        assert back.params.degree == 4 and back.params.two_phase == two_phase
    with pytest.raises(ValueError, match="exactly one"):
        ss.search(g, 10, 20, degree=4)
    with pytest.raises(ValueError, match="two_phase needs degree"):
        ss.search(g, 10, 20, two_phase=True)
    with pytest.raises(ValueError, match="unweighted"):
        ss.search(ss.WeightedGraph.from_graph(g), 10, degree=4)


def test_legacy_regular_programs():
    strip = lambda s: re.sub(r"T> .*", "T>", s)  # noqa: E731
    for program in ("reg_search", "ereg_search"):
        for name in ["paley_4reg", "paley_6reg", "dense_5reg", "doc_4cycles", "geometric"]:
            out, status = getattr(ss.legacy, program)((FIXTURES / program / f"{name}.in").read_text())
            assert strip(out) == strip((FIXTURES / program / f"{name}.out").read_text()), (program, name)
            assert status == 0


def test_regular_search_matches_c_through_the_modern_api():
    # rng="legacy" reproduces reg_search's experiments: the same subgraphs, in order
    g = paley(61)
    r = ss.search(g, 10, degree=4, experiments=20, max_moves=2000, div_freq=300, div_dur=8, seed=13, rng="legacy")
    expected = (FIXTURES / "reg_search" / "paley_4reg.out").read_text()
    blocks = [b.strip() for b in expected.split("\n\n") if re.fullmatch(r"[0-9 ]+", b.strip()) and len(b.split()) == 10]
    assert [" ".join(map(str, e.vertices)) for e in r.experiments] == blocks


def test_command_line_ships_with_the_package(tmp_path):
    # the `subgraphsearch` command installed by pip runs the Rust CLI inside the extension
    import subprocess
    import sys

    def cli(*args):
        return subprocess.run([sys.executable, "-m", "subgraphsearch._cli", *args], capture_output=True, text=True)

    assert cli("--version").stdout.strip() == f"subgraphsearch {ss.__version__}"
    path = tmp_path / "g.g6"
    assert cli("generate", "exponential", "-n", "100", "-d", "0.6", "-s", "1", "-o", str(path)).returncode == 0
    found = cli("search", str(path), "-k", "11", "-m", "55", "--json", str(tmp_path / "r.json"))
    assert found.returncode == 0 and "40/40 experiments" in found.stdout
    assert json.loads((tmp_path / "r.json").read_text())["experiments"]
    assert cli("search", str(path), "-k", "11", "-m", "900", "-n", "2", "--moves", "20", "-q").returncode == 2
    assert cli("no-such-command").returncode == 2
    converted = cli("convert", str(path), "-", "--to", "edgelist")
    assert converted.returncode == 0 and converted.stdout.startswith("# vertices: 100")


def test_quickstart_notebook_ships_with_the_package(tmp_path):
    from subgraphsearch.__main__ import main

    assert main(["quickstart", str(tmp_path), "--no-open"]) == 0
    packaged = json.loads((tmp_path / "quickstart.ipynb").read_text())
    executed = json.loads((Path(__file__).resolve().parents[2] / "examples" / "quickstart.ipynb").read_text())
    # the same cells as the executed notebook in the repo (rerun examples/make_quickstart.py), no outputs
    assert [c["source"] for c in packaged["cells"]] == [c["source"] for c in executed["cells"]]
    assert all(not c.get("outputs") for c in packaged["cells"])
    # an existing notebook, perhaps edited, is never overwritten
    (tmp_path / "quickstart.ipynb").write_text("mine")
    assert main(["quickstart", str(tmp_path), "--no-open"]) == 0
    assert (tmp_path / "quickstart.ipynb").read_text() == "mine"
