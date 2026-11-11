# Benchmark data

These files hold the benchmark instances behind the README's comparison with other methods,
which the quickstart notebook also runs. Each set is
distributed under its own license, **not** under the MIT license that covers the rest of this
project, and none of them is part of the Python or Rust packages.

## DIMACS clique benchmark graphs

`dimacs.g6` holds eight graphs from the [DIMACS](http://dimacs.rutgers.edu/) maximum-clique
benchmark, one per line in [graph6](https://users.cecs.anu.edu.au/~bdm/data/formats.html) format,
in this order:

| Line | Graph | Vertices | Edges | Largest clique |
| --- | --- | --- | --- | --- |
| 1 | C125.9 | 125 | 6,963 | 34 |
| 2 | C250.9 | 250 | 27,984 | 44 |
| 3 | p_hat300-3 | 300 | 33,390 | 36 |
| 4 | MANN_a27 | 378 | 70,551 | 126 |
| 5 | hamming8-4 | 256 | 20,864 | 16 |
| 6 | keller4 | 171 | 9,435 | 11 |
| 7 | brock200_2 | 200 | 9,876 | 12 |
| 8 | brock400_2 | 400 | 59,786 | 29 |

### Where the graphs come from

The graphs come from the [Network Repository](https://networkrepository.com/dimacs), where they
are distributed in Matrix Market format under the
[Creative Commons Attribution-ShareAlike 4.0 license](https://creativecommons.org/licenses/by-sa/4.0/)
(see the repository's [data license and policy](https://networkrepository.com/policy.php)).
`dimacs.g6` is distributed under the same license.

Changes: the eight graphs were converted from Matrix Market to graph6 and combined into one file.
The vertices are numbered from 0 in the original order; the graphs are otherwise unchanged.

Please cite the Network Repository when using these graphs:

> Ryan A. Rossi and Nesreen K. Ahmed. The Network Data Repository with Interactive Graph
> Analytics and Visualization. In *Proceedings of the Twenty-Ninth AAAI Conference on Artificial
> Intelligence*, 2015. <https://networkrepository.com>

## MDPLIB maximum diversity instances

`mdplib/` holds eight SOM-b instances of the maximum diversity problem from MDPLIB: choose `m` of `n`
items to maximize the sum of the distances between the chosen ones. Each file's first line is
`n m`, and each further line `i j d` gives the distance `d`, an integer from 0 to 9, between items
`i` and `j`, numbered from 0.

| File | Items | Chosen | Best known value |
| --- | --- | --- | --- |
| SOM-b_1_n100_m10.txt | 100 | 10 | 333 |
| SOM-b_2_n100_m20.txt | 100 | 20 | 1,195 |
| SOM-b_3_n100_m30.txt | 100 | 30 | 2,457 |
| SOM-b_4_n100_m40.txt | 100 | 40 | 4,142 |
| SOM-b_6_n200_m40.txt | 200 | 40 | 4,450 |
| SOM-b_10_n300_m60.txt | 300 | 60 | 9,689 |
| SOM-b_13_n400_m40.txt | 400 | 40 | 4,658 |
| SOM-b_14_n400_m80.txt | 400 | 80 | 16,956 |

The best known values are those MDPLIB lists, from two-hour runs of scatter search.

### Where the instances come from

The instances come from MDPLIB 2010 (Martí, Gallego and Duarte, Optsicom project, University of
Valencia; [problem page](https://grafo.etsii.urjc.es/optsicom/mdp.html)), distributed on
[Zenodo](https://doi.org/10.5281/zenodo.13749317) under the
[Creative Commons Attribution 4.0 license](https://creativecommons.org/licenses/by/4.0/). The files
in `mdplib/` are distributed under the same license.

Changes: none; the eight files are copied unchanged from the MDPLIB archive.

Please cite MDPLIB when using these instances:

> Rafael Martí, Micael Gallego and Abraham Duarte. A branch and bound algorithm for the maximum
> diversity problem. *European Journal of Operational Research* 200(1), 36–44, 2010.
> MDPLIB: <https://doi.org/10.5281/zenodo.13749317>
