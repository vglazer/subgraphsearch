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
    assert "in_subgraph" in (tmp_path / "g.graphml").read_text(encoding="utf-8")
    assert ss.formats()["dot"] is False


def test_legacy_graphs_match_ggen():
    lists = (FIXTURES / "graphs" / "exp_100_600_1.txt").read_text(encoding="utf-8")
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
    strip = lambda s: re.sub(r"T> .*", "T>", s)
    for name in ["readme_cliques", "fixed_cliques", "approx_adj", "complement"]:
        out, status = ss.legacy.sub_search((FIXTURES / "sub_search" / f"{name}.in").read_text(encoding="utf-8"))
        assert strip(out) == strip((FIXTURES / "sub_search" / f"{name}.out").read_text(encoding="utf-8")), name
        assert status == 0


def test_legacy_asub_search_matches_the_c_program_fixtures():
    strip = lambda s: re.sub(r"T> .*", "T>", s)
    for name in ["readme_cliques", "one_step_show", "approx_adj", "geometric"]:
        out, status = ss.legacy.asub_search((FIXTURES / "asub_search" / f"{name}.in").read_text(encoding="utf-8"))
        assert strip(out) == strip((FIXTURES / "asub_search" / f"{name}.out").read_text(encoding="utf-8")), name
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
    assert ss.legacy.ggen((FIXTURES / "ggen" / "exp_10_500_42.in").read_text(encoding="utf-8")) == (FIXTURES / "ggen" / "exp_10_500_42.out").read_text(encoding="utf-8")
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
    text = Path(path).read_text(encoding="utf-8")
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


def test_shape_checks():
    path = ss.Graph(5, [(0, 1), (1, 2), (2, 3), (3, 4)])
    cycle = ss.Graph(5, [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)])
    star = ss.Graph(5, [(0, 1), (0, 2), (0, 3), (0, 4)])
    forest = ss.Graph(5, [(0, 1), (2, 3)])
    k4 = ss.Graph(4, [(u, v) for u in range(4) for v in range(u + 1, 4)])
    checks = ("is_connected", "is_tree", "is_path", "is_cycle", "is_clique", "is_independent")
    expected = {
        "path": (path, (True, True, True, False, False, False)),
        "cycle": (cycle, (True, False, False, True, False, False)),
        "star": (star, (True, True, False, False, False, False)),
        "forest": (forest, (False, False, False, False, False, False)),
        "k4": (k4, (True, False, False, False, True, False)),
        "empty": (ss.Graph(3), (False, False, False, False, False, True)),
        "one vertex": (ss.Graph(1), (True, True, True, False, True, True)),
        "no vertices": (ss.Graph(0), (False, False, False, False, True, True)),
    }
    for name, (g, answers) in expected.items():
        assert tuple(getattr(g, c)() for c in checks) == answers, name
    # on a subgraph a search found
    grid = ss.grid(6, 6)
    r = ss.search(grid, 12, path=True, experiments=20, max_moves=1000)
    assert all(grid.induced(vs).is_path() for vs in r.solutions())


def test_graphs_of_vectors():
    import itertools

    # all +-1 vectors of length 4 starting with +1: orthogonal pairs, and the order-4 Hadamard matrix as a clique
    v = np.array([(1,) + s for s in itertools.product((1, -1), repeat=3)])
    g = ss.Graph.from_vectors(v, [0])
    dots = v @ v.T
    assert sorted(g.edge_list()) == [(i, j) for i in range(8) for j in range(i + 1, 8) if dots[i, j] == 0]
    r = ss.search(g, 4, 6, experiments=10)
    h = v[list(r.best.vertices)]
    assert r.successes and (h @ h.T == 4 * np.eye(4)).all() and g.induced(r.best.vertices).is_clique()
    # several inner products, and plain lists
    assert ss.Graph.from_vectors([[1, 0], [0, 1], [1, 1]], [0, 1]).m == 3
    assert ss.Graph.from_vectors(np.zeros((0, 3), dtype=int), [0]).n == 0
    with pytest.raises(ValueError, match="2-D"):
        ss.Graph.from_vectors(np.ones(4, dtype=int), [0])
    with pytest.raises(ValueError, match="integer"):
        ss.Graph.from_vectors(np.ones((2, 2)) / 2, [0])

    # the matrix plot: the order-4 Hadamard matrix found above
    ax = viz.plot_sign_matrix(h)
    assert ax.images and "sign pattern" in ax.get_title(loc="left")
    with pytest.raises(ValueError, match="2-D"):
        viz.plot_sign_matrix(np.ones(3))
    matplotlib.pyplot.close("all")


def test_words_and_codes():
    v = ss.vectors
    words = v.binary_words(4)
    assert words.shape == (16, 4) and words[5].tolist() == [1, 0, 1, 0]   # word k spells k, bit i first
    assert v.binary_words(5, weight=2).shape == (10, 5) and (v.binary_words(5, weight=2).sum(1) == 2).all()
    assert (v.signs([[0, 1, 1]]) == [[1, -1, -1]]).all()
    with pytest.raises(ValueError, match="weight"):
        v.binary_words(4, weight=5)

    # from_words agrees with from_vectors on the same words as +-1 vectors (inner product n - 2t)
    n, d = 7, 3
    words = v.binary_words(n)
    far = ss.Graph.from_words(words, range(d, n + 1))
    assert far == ss.Graph.from_vectors(v.signs(words), range(-n, n - 2 * d + 1, 2))
    assert ss.Graph.from_words(words, range(1, d)) == far.complement()
    # the Hamming code of length 7: 16 words at distance 3, as a clique
    r = ss.search(far, 16, 120, experiments=16, max_moves=2000)
    code = words[list(r.best.vertices)]
    assert r.successes and v.min_distance(code) == 3
    assert v.supports([[0, 1, 1, 0]]) == [(2, 3)] and v.supports([[0, 1, 1, 0]], base=0) == [(1, 2)]
    with pytest.raises(ValueError, match="0 and 1"):
        ss.Graph.from_words([[0, 2]], [1])
    with pytest.raises(ValueError, match="64 bits"):
        ss.Graph.from_words(np.zeros((2, 65), dtype=int), [1])

    # the same code as an independent set in the "too close" graph, with region kicks of 4 codewords
    close = ss.Graph.from_words(words, range(1, d))
    kicked = ss.search(close, 16, 0, kick_size=4, experiments=16, max_moves=2000, div_freq=100)
    assert kicked.successes and kicked.params.kick_size == 4
    assert all(v.min_distance(words[list(vs)]) >= d for vs in kicked.solutions())
    assert ss.SearchResult.from_dict(json.loads(kicked.to_json()), close).params.kick_size == 4
    assert ss.largest(close, "independent", kick_size=4, experiments=16, max_moves=2000, div_freq=100).size == 16
    with pytest.raises(ValueError, match="edge-count searches only"):
        ss.search(close, 8, path=True, kick_size=4)


    # Hadamard checks
    assert v.is_hadamard([[1, 1], [1, -1]]) and not v.is_hadamard([[1, 1], [1, 1]]) and not v.is_hadamard([[1, 0]])


def test_graph_products():
    complete = lambda n: ss.Graph(n).complement()
    path = lambda n: ss.Graph(n, [(i, i + 1) for i in range(n - 1)])
    rook = ss.Graph(16, [(a, b) for a in range(16) for b in range(a + 1, 16) if a // 4 == b // 4 or a % 4 == b % 4])
    assert complete(4).cartesian(complete(4)) == rook
    assert path(3).cartesian(path(4)) == ss.grid(3, 4)
    assert complete(2).cartesian(complete(2)).cartesian(complete(2)) == ss.hypercube(3)
    assert complete(2).tensor(complete(2)).m == 2 and complete(2).strong(complete(2)) == complete(4)
    assert path(8).strong(path(8)).m == 210                       # the king's graph
    assert path(3).lexicographic(complete(2)).m == 2 * 4 + 3
    truncated = complete(4).replacement(complete(3))              # the truncated tetrahedron
    assert (truncated.n, truncated.m) == (12, 18) and set(truncated.degrees().tolist()) == {3}
    z = complete(5).zigzag(ss.Graph(4, [(0, 1), (1, 2), (2, 3), (3, 0)]))
    assert z.n == 20 and set(z.degrees().tolist()) == {4}
    with pytest.raises(ValueError, match="as many vertices"):
        complete(4).replacement(complete(4))
    with pytest.raises(ValueError, match="regular"):
        path(3).zigzag(complete(2))


def test_standard_graphs_and_mean_degree():
    n = 6
    by_hand = ss.Graph(2 ** n, [(u, u ^ (1 << b)) for u in range(2 ** n) for b in range(n) if u < u ^ (1 << b)])
    assert ss.hypercube(6) == by_hand
    grid = ss.Graph(36, [(i, i + 1) for i in range(36) if i % 6 < 5] + [(i, i + 6) for i in range(30)])
    assert ss.grid(6, 6) == grid
    with pytest.raises(ValueError, match="16 dimensions"):
        ss.hypercube(17)

    g = ss.generate("exponential", 50_000, mean_degree=3, seed=2)
    assert g.n == 50_000 and abs(2 * g.m / g.n - 3) < 0.05
    assert ss.generate("exponential", 50_000, mean_degree=3, seed=2) == g
    assert ss.generate("exponential", 20, mean_degree=19).m == 190
    with pytest.raises(ValueError, match="exactly one"):
        ss.generate("exponential", 20, 0.5, mean_degree=3)
    with pytest.raises(ValueError, match="exactly one"):
        ss.generate("exponential", 20)
    with pytest.raises(ValueError, match="xoshiro"):
        ss.generate("power", 20, mean_degree=3)
    with pytest.raises(ValueError, match="mean degree"):
        ss.generate("exponential", 20, mean_degree=25)

    # random regular graphs: every degree right, reproducible from the seed, and good expanders
    r = ss.random_regular(64, 8, seed=1)
    assert set(r.degrees().tolist()) == {8} and r == ss.random_regular(64, 8, seed=1) and r != ss.random_regular(64, 8, seed=2)
    eigenvalues = np.linalg.eigvalsh(r.adjacency_matrix(dtype=float))
    assert max(eigenvalues[-2], -eigenvalues[0]) < 2 * np.sqrt(7) + 0.5
    assert set(ss.random_regular(20, 15).degrees().tolist()) == {15}
    with pytest.raises(ValueError, match="no 3-regular"):
        ss.random_regular(7, 3)


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
    pytest.importorskip("networkx")
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
    expected = (FIXTURES / "wsub_search" / "light.out").read_text(encoding="utf-8")
    for e in legacy.experiments:
        assert " ".join(map(str, e.vertices)) + " " in expected


def test_legacy_weighted_programs():
    strip = lambda s: re.sub(r"T> .*", "T>", s)
    for name in ["light_adj", "multi_weights", "hard_183", "fixed", "unit_weights"]:
        out, _ = ss.legacy.wsub_search((FIXTURES / "wsub_search" / f"{name}.in").read_text(encoding="utf-8"))
        assert strip(out) == strip((FIXTURES / "wsub_search" / f"{name}.out").read_text(encoding="utf-8")), name
    for name in ["w10_50_3", "w60_150_3"]:
        args = (FIXTURES / "wggen" / f"{name}.args").read_text(encoding="utf-8")
        assert ss.legacy.wggen(args) == (FIXTURES / "wggen" / f"{name}.out").read_text(encoding="utf-8")
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


def test_connected_search():
    # two triangles and a 6-cycle: of the subgraphs on 6 vertices with 6 edges, only the 6-cycle is connected
    g = ss.Graph(12, [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)] + [(i, 6 + (i - 5) % 6) for i in range(6, 12)])
    for kwargs in ({"edges": 6}, {"degree": 2}, {"degree": 2, "two_phase": True}):
        r = ss.search(g, 6, **kwargs, connected=True, experiments=30, max_moves=500, div_freq=50, trace=True)
        assert r.successes > 0 and r.solutions() == [tuple(range(6, 12))]
        assert r.target == ("with 6 edges, connected" if "edges" in kwargs else "forming a cycle")
        assert r.target in r.summary()
        back = ss.SearchResult.from_dict(json.loads(r.to_json()), g)
        assert back.params.connected
        viz.plot_search_summary(r)
    plain = ss.search(g, 6, degree=2, experiments=30, max_moves=500, div_freq=50)
    assert not plain.params.connected and len(plain.solutions()) == 2
    assert ss.search(g, 5, 4, connected=True).target == "forming a tree"


def is_induced_path(g, vertices):
    h = g.induced(vertices)
    return h.components()[1] == 1 and h.m == len(vertices) - 1 and h.degrees().max() <= 2


def test_path_search():
    # a 6x6 grid: plenty of induced paths, and 4-cycles that a search for degree 2 would end on
    grid = ss.Graph(36, [(i, i + 1) for i in range(36) if i % 6 < 5] + [(i, i + 6) for i in range(30)])
    for k in (5, 12):
        r = ss.search(grid, k, path=True, experiments=30, max_moves=1000, seed=2, trace=True)
        assert r.successes > 0 and r.params.path and r.target == "forming a path" and "path=True" in repr(r)
        assert all(is_induced_path(grid, e.vertices) for e in r.experiments if e.success)
        assert all(min(e.trace) == e.best_conflict for e in r.experiments)
        assert ss.SearchResult.from_dict(json.loads(r.to_json()), grid).params.path
        viz.plot_search_summary(r)
    through = ss.search(grid, 8, path=True, fixed=[14], experiments=30, max_moves=1000)
    assert through.successes > 0 and all(14 in vs for vs in through.solutions())
    with pytest.raises(ValueError, match="exactly one"):
        ss.search(grid, 8, 7, path=True)
    with pytest.raises(ValueError, match="exactly one"):
        ss.search(grid, 8, degree=2, path=True)
    with pytest.raises(ValueError, match="unweighted"):
        ss.search(ss.WeightedGraph.from_graph(grid), 8, path=True)


def test_path_search_between_endpoints(tmp_path):
    import subprocess
    import sys

    # opposite corners of a 6x6 grid, 10 steps apart: the paths between them have an odd number of vertices
    grid = ss.Graph(36, [(i, i + 1) for i in range(36) if i % 6 < 5] + [(i, i + 6) for i in range(30)])
    r = ss.search(grid, 15, path=True, endpoints=(0, 35), experiments=30, max_moves=1000)
    assert r.successes > 0 and r.params.endpoints == (0, 35) and r.target == "forming a path from 0 to 35"
    for e in r.experiments:
        assert len(e.vertices) == 15 and {0, 35} <= set(e.vertices)
        if e.success:
            h = grid.induced(list(e.vertices))
            ends = [v for v, d in zip(e.vertices, h.degrees().tolist()) if d == 1]
            assert is_induced_path(grid, e.vertices) and ends == [0, 35]
    assert ss.SearchResult.from_dict(json.loads(r.to_json()), grid).params.endpoints == (0, 35)
    assert ss.search(grid, 14, path=True, endpoints=(0, 35), experiments=30, max_moves=1000).successes == 0
    with pytest.raises(ValueError, match="path search"):
        ss.search(grid, 8, 7, endpoints=(0, 35))
    with pytest.raises(ValueError, match="two vertices"):
        ss.search(grid, 8, path=True, endpoints=(0, 7, 35))

    # the command line takes them too, and its JSON loads in Python
    graph_path, out = tmp_path / "grid.g6", tmp_path / "r.json"
    grid.write(graph_path)
    run = subprocess.run(
        [sys.executable, "-m", "subgraphsearch._cli", "search", str(graph_path), "-k", "15", "--path",
         "--endpoints", "0,35", "-n", "30", "--moves", "1000", "--json", str(out), "-q"],
        check=False, capture_output=True, text=True, encoding="utf-8",
    )
    assert "forming a path from 0 to 35" in run.stdout
    assert [e.vertices for e in ss.SearchResult.load(out, grid).experiments] == [e.vertices for e in r.experiments]


def test_largest(tmp_path):
    import subprocess
    import sys

    n = 6
    cube = ss.Graph(2 ** n, [(u, u ^ (1 << b)) for u in range(2 ** n) for b in range(n) if u < u ^ (1 << b)])
    settings = {"experiments": 50, "max_moves": 2000, "div_freq": 300, "div_dur": 8}
    # the 6-cube's longest snake has 27 vertices and its longest coil 26; it has no triangles
    # and it is bipartite with two parts of 32, so its largest independent sets have 32 vertices
    for shape, size in [("path", 27), ("cycle", 26), ("clique", 2), ("independent", 32)]:
        r = ss.largest(cube, shape, **settings)
        assert r.size == size and r.best.params.size == size and r.best.successes > 0, shape
        assert list(r.results) == sorted(r.results) and all(x.successes == 0 for k, x in r.results.items() if k > size)
        assert r.next.params.size > size and r.next.successes == 0
        assert f"{size} vertices" in r.summary() and f"size={size}" in repr(r)
    cycles = ss.largest(cube, "cycle", **settings)
    assert cycles.best.target == "forming a cycle"
    assert all(is_induced_path(cube, vs) for vs in ss.largest(cube, "path", **settings).best.solutions())
    assert all(cube.induced(vs).is_independent() for vs in ss.largest(cube, "independent", **settings).best.solutions())
    trees = ss.largest(cube, "tree", **settings)
    assert trees.size >= 27 and trees.best.target == "forming a tree"

    # between opposite corners it starts at the shortest path, 7 vertices, and every path has an odd number
    corner = ss.largest(cube, "path", endpoints=(0, 63), **settings)
    assert min(corner.results) == 7 and corner.size % 2 == 1 and corner.size >= 21
    assert all({0, 63} <= set(vs) for vs in corner.best.solutions())
    assert corner.best.params.endpoints == (0, 63)
    with pytest.raises(ValueError, match="path search"):
        ss.largest(cube, "cycle", endpoints=(0, 63))
    with pytest.raises(ValueError, match="unknown shape"):
        ss.largest(cube, "star")
    with pytest.raises(TypeError, match="unweighted"):
        ss.largest(ss.WeightedGraph.from_graph(cube), "path")

    # the command line finds the same
    graph_path = tmp_path / "cube.g6"
    cube.write(graph_path)
    run = subprocess.run(
        [sys.executable, "-m", "subgraphsearch._cli", "largest", str(graph_path), "--shape", "cycle", "-n", "50",
         "--moves", "2000", "--div-freq", "300", "--div-dur", "8", "-q", "--json", str(tmp_path / "l.json")],
        check=False, capture_output=True, text=True, encoding="utf-8",
    )
    assert run.returncode == 0 and "largest induced cycle found: 26 vertices" in run.stdout
    saved = json.loads((tmp_path / "l.json").read_text(encoding="utf-8"))
    assert saved["shape"] == "cycle" and saved["size"] == 26


def test_legacy_regular_programs():
    strip = lambda s: re.sub(r"T> .*", "T>", s)
    for program in ("reg_search", "ereg_search"):
        for name in ["paley_4reg", "paley_6reg", "dense_5reg", "doc_4cycles", "geometric"]:
            out, status = getattr(ss.legacy, program)((FIXTURES / program / f"{name}.in").read_text(encoding="utf-8"))
            assert strip(out) == strip((FIXTURES / program / f"{name}.out").read_text(encoding="utf-8")), (program, name)
            assert status == 0


def test_regular_search_matches_c_through_the_modern_api():
    # rng="legacy" reproduces reg_search's experiments: the same subgraphs, in order
    g = paley(61)
    r = ss.search(g, 10, degree=4, experiments=20, max_moves=2000, div_freq=300, div_dur=8, seed=13, rng="legacy")
    expected = (FIXTURES / "reg_search" / "paley_4reg.out").read_text(encoding="utf-8")
    blocks = [b.strip() for b in expected.split("\n\n") if re.fullmatch(r"[0-9 ]+", b.strip()) and len(b.split()) == 10]
    assert [" ".join(map(str, e.vertices)) for e in r.experiments] == blocks


def test_command_line_ships_with_the_package(tmp_path):
    # the `subgraphsearch` command installed by pip runs the Rust CLI inside the extension
    import subprocess
    import sys

    def cli(*args):
        return subprocess.run([sys.executable, "-m", "subgraphsearch._cli", *args], check=False, capture_output=True, text=True, encoding="utf-8")

    assert cli("--version").stdout.strip() == f"subgraphsearch {ss.__version__}"
    path = tmp_path / "g.g6"
    assert cli("generate", "exponential", "-n", "100", "-d", "0.6", "-s", "1", "-o", str(path)).returncode == 0
    found = cli("search", str(path), "-k", "11", "-m", "55", "--json", str(tmp_path / "r.json"))
    assert found.returncode == 0 and "40/40 experiments" in found.stdout
    assert json.loads((tmp_path / "r.json").read_text(encoding="utf-8"))["experiments"]
    assert cli("search", str(path), "-k", "11", "-m", "900", "-n", "2", "--moves", "20", "-q").returncode == 2
    assert cli("no-such-command").returncode == 2
    converted = cli("convert", str(path), "-", "--to", "edgelist")
    assert converted.returncode == 0 and converted.stdout.startswith("# vertices: 100")


def test_quickstart_notebook_ships_with_the_package(tmp_path):
    from subgraphsearch.__main__ import main

    assert main(["quickstart", str(tmp_path), "--no-open"]) == 0
    packaged = json.loads((tmp_path / "quickstart.ipynb").read_text(encoding="utf-8"))
    executed = json.loads((Path(__file__).resolve().parents[2] / "examples" / "quickstart.ipynb").read_text(encoding="utf-8"))
    # the same cells as the executed notebook in the repo (rerun examples/make_quickstart.py), no outputs
    assert [c["source"] for c in packaged["cells"]] == [c["source"] for c in executed["cells"]]
    assert all(not c.get("outputs") for c in packaged["cells"])
    # an existing notebook, perhaps edited, is kept unless --force replaces it
    (tmp_path / "quickstart.ipynb").write_text("mine", encoding="utf-8")
    assert main(["quickstart", str(tmp_path), "--no-open"]) == 0
    assert (tmp_path / "quickstart.ipynb").read_text(encoding="utf-8") == "mine"
    assert main(["quickstart", str(tmp_path), "--no-open", "--force"]) == 0
    assert json.loads((tmp_path / "quickstart.ipynb").read_text(encoding="utf-8")) == packaged


def srg_parameters(g):
    """(n, k, λ, μ) if g is strongly regular, else None."""
    a = g.adjacency_matrix().astype(int)
    common, n = a @ a, len(a)
    degrees = set(a.sum(axis=1).tolist())
    lam = {int(common[i, j]) for i in range(n) for j in range(i + 1, n) if a[i, j]}
    mu = {int(common[i, j]) for i in range(n) for j in range(i + 1, n) if not a[i, j]}
    return (n, *degrees, *lam, *mu) if len(degrees) == len(lam) == len(mu) == 1 else None


def test_incidence_graphs_and_chang_graphs():
    import networkx as nx

    fano = [(0, 1, 2), (0, 3, 4), (0, 5, 6), (1, 3, 5), (1, 4, 6), (2, 3, 6), (2, 4, 5)]
    assert ss.Graph.point_graph(fano).num_edges == 21          # every two points share a line
    assert ss.Graph.block_graph(fano).num_edges == 21          # every two lines meet in a point
    assert ss.Graph.point_graph(fano, k=2).num_edges == 0
    assert ss.Graph.point_graph([(0, 1)], points=5).num_vertices == 5
    with pytest.raises(ValueError):
        ss.Graph.point_graph([(0, 1)], points=1)

    # the block graph of all pairs from 8 points is the triangular graph T(8), srg(28, 12, 6, 4);
    # switching it with respect to four disjoint pairs (an independent 4-set the search finds)
    # gives a Chang graph: strongly regular with the same parameters, but not isomorphic to T(8)
    t8 = ss.Graph.block_graph([(i, j) for i in range(8) for j in range(i + 1, 8)])
    assert srg_parameters(t8) == (28, 12, 6, 4)
    matching = ss.search(t8, 4, 0, experiments=10).best.vertices
    chang = t8.switch(matching)
    assert srg_parameters(chang) == (28, 12, 6, 4)
    assert not nx.is_isomorphic(convert.to_networkx(t8), convert.to_networkx(chang))


def test_fano_plane_and_switching_drawings():
    import matplotlib.pyplot as plt

    fano = ss.designs.FANO_PLANE
    # a (7, 3, 1) design: 7 lines of 3 points, every two points on exactly one line
    assert len(fano) == 7 and all(len(line) == 3 for line in fano)
    assert all(sum(a in line and b in line for line in fano) == 1 for a in range(7) for b in range(a + 1, 7))
    ax = viz.plot_fano_plane()
    assert len(ax.lines) == 6 and len(ax.patches) == 1   # six straight lines and the circle
    plt.close("all")

    pairs = [(i, j) for i in range(8) for j in range(i + 1, 8)]
    t8 = ss.Graph.block_graph(pairs)
    sets = [[0, 13, 22, 27], list(range(8))]
    fig = viz.plot_switchings(t8, sets, titles=["a", "b"], above=[t8, t8, t8])
    assert len(fig.axes) == 6
    assert len(viz.plot_switchings(t8, sets).axes) == 3
    with pytest.raises(ValueError):
        viz.plot_switchings(t8, sets, above=[t8])
    plt.close("all")


def test_kick_default():
    g = ss.generate("exponential", 60, 0.5, seed=3)
    # edge-count and weight searches kick by default; path, regular and legacy searches do not
    assert ss.search(g, 10, 20).params.kick_size == ss.DEFAULT_KICK_SIZE == 4
    assert ss.search(ss.generate_weighted(40, 0.5), 5, 4).params.kick_size == 4
    assert ss.search(g, 6, path=True).params.kick_size == 0
    assert ss.search(g, 6, degree=2).params.kick_size == 0
    assert ss.search(g, 10, 20, rng="legacy").params.kick_size == 0
    assert ss.largest(g, "cycle").results[3].params.kick_size == 0
    assert ss.largest(g, "clique").best.params.kick_size == 4
    # within the first div_freq moves kicks change nothing, so default-length searches are as before
    short = {"experiments": 20, "max_moves": 250, "div_freq": 250}
    assert ss.search(g, 15, 60, **short).experiments == ss.search(g, 15, 60, kick_size=0, **short).experiments


def test_targets_above_the_total_find_the_closest_subgraphs():
    # the weighted graph's total weight is far below the target, yet the search still finds the
    # heaviest subgraphs it can, as near misses
    w = ss.generate_weighted(60, 0.5, seed=2)
    target = w.total_weight + 100
    best = ss.search(w, 8, target, max_moves=1000).best
    assert best.best_conflict == target - w.induced_weight(best.vertices)
    assert w.induced_weight(best.vertices) > 0
