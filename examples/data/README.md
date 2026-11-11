# DIMACS clique benchmark graphs

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

The quickstart notebook's comparison with other methods uses them.

## Source, license and changes

The graphs come from the [Network Repository](https://networkrepository.com/dimacs), where they
are distributed in Matrix Market format under the
[Creative Commons Attribution-ShareAlike 4.0 license](https://creativecommons.org/licenses/by-sa/4.0/)
(see the repository's [data license and policy](https://networkrepository.com/policy.php)).
This file is distributed under the same license, **not** under the MIT license that covers the
rest of this project.

Changes: the eight graphs were converted from Matrix Market to graph6 and combined into one file.
The vertices are numbered from 0 in the original order; the graphs are otherwise unchanged.

Please cite the Network Repository when using these graphs:

> Ryan A. Rossi and Nesreen K. Ahmed. The Network Data Repository with Interactive Graph
> Analytics and Visualization. In *Proceedings of the Twenty-Ninth AAAI Conference on Artificial
> Intelligence*, 2015. https://networkrepository.com
