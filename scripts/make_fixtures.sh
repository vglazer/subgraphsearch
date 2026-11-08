#!/usr/bin/env bash
# Regenerate the golden fixtures used to check that `subgraphsearch legacy ...` reproduces the
# original C programs byte for byte. Requires a clone of the repository holding the original
# (https://github.com/vglazer/USRA/tree/master/subgraph_finding); see PORTING.md.
#
# Usage: ORIGINAL_DIR=~/repos/USRA scripts/make_fixtures.sh

set -euo pipefail

original_dir=${ORIGINAL_DIR:?set ORIGINAL_DIR to a clone of the repository holding the original}
src="$original_dir/subgraph_finding/src"
repo_dir=$(cd "$(dirname "$0")/.." && pwd)
out="$repo_dir/tests/fixtures"
bin=$(mktemp -d)
trap 'rm -rf "$bin"' EXIT

for prog in sub_search asub_search wsub_search reg_search ereg_search ggen wggen; do
  gcc -ansi -pedantic -Wall -O2 "$src/$prog.c" -o "$bin/$prog"
done

mkdir -p "$out/ggen" "$out/wggen" "$out/sub_search" "$out/asub_search" "$out/wsub_search" "$out/reg_search" "$out/ereg_search" "$out/graphs"
echo "Original commit: $(git -C "$original_dir" rev-parse HEAD)" > "$out/SOURCE"

# ---- ggen: name | stdin
ggen_case() {
  local name=$1; shift
  printf '%b' "$1" > "$out/ggen/$name.in"
  # errors go to stderr; record exit status so error cases are checked too
  set +e
  "$bin/ggen" < "$out/ggen/$name.in" > "$out/ggen/$name.out" 2> "$out/ggen/$name.err"
  echo $? > "$out/ggen/$name.status"
  set -e
}

ggen_case exp_10_0_42        "2 10 0 0 42 0 0 0\n"
ggen_case exp_10_500_42      "2 10 0 500 42 0 0 0\n"
ggen_case exp_100_600_1      "2 100 0 600 1 0 0 0\n"
ggen_case exp_100_600_2_c    "2 100 0 600 2 0 0 1\n"
ggen_case exp_37_1000_11     "2 37 0 1000 11 0 0 0\n"
ggen_case pow_10_500_42      "3 10 0 500 42 0 0 0\n"
ggen_case pow_300_200_52_c   "3 300 0 200 52 0 0 1\n"
ggen_case pow_37_1000_11     "3 37 0 1000 11 0 0 0\n"
ggen_case pow_1_500_3        "3 1 0 500 3 0 0 0\n"
ggen_case geo_10_500_42      "4 10 0 500 42 0 0 0\n"
ggen_case geo_790_150_1      "4 790 0 150 1 0 0 0\n"
ggen_case empty_0            "2 0 0 500 1 0 0 0\n"

# operations on an existing adjacency list
"$bin/ggen" <<< "2 40 0 450 3 0 0 0" | grep -- '-1$' > "$bin/g40.txt"
for spec in "4 1 0:17 5 28 9" "4 1 1:17 5 28 9" "4 -1 0:17 5 28 9" "4 2 0:17 5 28 9" \
            "4 -2 1:17 5 28 9" "4 3 0:17 5 28 9" "1 2 0:7" "1 -2 0:7"; do
  params=${spec%%:*}; fixed=${spec#*:}
  name="adj_$(echo "$params" | tr ' -' '_m')"
  ggen_case "$name" "0 40 0 0 0 $params\n$(cat "$bin/g40.txt")\n$fixed\n"
done
ggen_case adj_plain      "0 40 0 0 0 0 0 0\n$(cat "$bin/g40.txt")\n"
ggen_case adj_compl      "0 40 0 0 0 0 0 1\n$(cat "$bin/g40.txt")\n"

# incidence lists: 6 sets over 12 symbols, primal and dual
inc="0 1 2 3 -1\n2 3 4 5 -1\n5 6 7 -1\n0 7 8 9 -1\n9 10 11 -1\n1 4 8 11 -1\n"
ggen_case inc_primal     "1 12 6 1 0 0 0 0\n$inc"
ggen_case inc_primal_fix "1 12 6 1 0 3 -1 1\n${inc}0 4 9\n"
ggen_case inc_dual       "1 12 -6 1 0 0 0 0\n$inc"
ggen_case inc_dual_fix   "1 12 -6 2 0 2 2 0\n${inc}1 3\n"

# error handling
ggen_case err_seed0      "2 6 0 500 0 0 0 0\n"
ggen_case err_type       "7 6 0 500 1 0 0 0\n"
ggen_case err_range      "0 5 0 0 0 0 0 0\n1 7 -1\n-1\n-1\n-1\n-1\n"
ggen_case err_truncated  "2 10 0 500\n"

# ---- graphs used by the sub_search cases
graph() { "$bin/ggen" <<< "$2" | grep -- '-1$' > "$out/graphs/$1.txt"; }
graph exp_100_600_1   "2 100 0 600 1 0 0 0"
graph exp_100_600_2   "2 100 0 600 2 0 0 0"
graph exp_67_500_26   "2 67 0 500 26 0 0 0"
graph exp_300_140_7   "2 300 0 140 7 0 0 0"
graph pow_300_200_52  "3 300 0 200 52 0 0 0"
graph pow_300_200_52c "3 300 0 200 52 0 0 1"
graph geo_400_150_1   "4 400 0 150 1 0 0 0"
graph exp_30_300_5    "2 30 0 300 5 0 0 0"

# ---- sub_search and asub_search (same input): name | graph | parameters | fixed vertices
sub_case() {
  local name=$1 g=$2 params=$3 fixed=${4:-}
  for prog in sub_search asub_search; do
    { echo "$params"; cat "$out/graphs/$g.txt"; [[ -n $fixed ]] && echo "$fixed"; } > "$out/$prog/$name.in"
    set +e
    "$bin/$prog" < "$out/$prog/$name.in" > "$out/$prog/$name.out" 2> "$out/$prog/$name.err"
    echo $? > "$out/$prog/$name.status"
    set -e
  done
}

sub_case readme_cliques    exp_100_600_1   "100 11 55 0 5 100 25 4 72 1"
sub_case one_step          exp_100_600_2   "100 8 0 0 60 100 25 4 1 0"
sub_case one_step_show     exp_100_600_2   "100 8 0 0 60 100 25 4 1 1"
sub_case approx_3          exp_100_600_2   "100 8 0 0 60 100 25 4 1 3"
sub_case approx_adj        exp_100_600_2   "100 8 0 0 20 100 25 4 1 -3"
sub_case fixed_cliques     exp_67_500_26   "67 4 6 2 2000 500 1 0 26 5" "61 63"
sub_case fixed_adj         exp_300_140_7   "300 47 153 1 6 2000 500 120 3782 -1" "172"
sub_case cocliques         pow_300_200_52  "300 20 0 0 30 1500 300 6 9 1"
sub_case complement        pow_300_200_52c "300 12 30 0 10 1000 200 5 4 -2"
sub_case geometric         geo_400_150_1   "400 15 40 0 15 800 100 10 13 2"
sub_case hard_12_clique    exp_100_600_1   "100 12 66 0 8 3000 300 8 5 1"
sub_case hard_13_clique    exp_100_600_1   "100 13 78 0 4 3000 250 20 6 -2"
sub_case no_diversify      exp_100_600_1   "100 10 30 0 30 200 1 0 5 1"
sub_case always_diversify  exp_30_300_5    "30 6 7 0 30 50 1 1 17 1"
sub_case zero_moves        exp_30_300_5    "30 6 7 0 4 0 10 2 3 1"
sub_case unreachable       exp_30_300_5    "30 6 900 0 3 40 10 2 3 1"
sub_case big_seed          exp_30_300_5    "30 6 7 0 30 60 15 3 4000000000 1"
sub_case err_size          exp_30_300_5    "30 30 7 0 3 40 10 2 3 1"
sub_case err_freq          exp_30_300_5    "30 6 7 0 3 40 0 2 3 1"

# ---- wggen (arguments, not stdin): name | arguments
wggen_case() {
  local name=$1; shift
  echo "$*" > "$out/wggen/$name.args"
  set +e
  # run under a fixed name: wggen prints argv[0] in its messages
  # shellcheck disable=SC2086
  (exec -a wggen "$bin/wggen" $* > "$out/wggen/$name.out" 2> "$out/wggen/$name.err")
  echo $? > "$out/wggen/$name.status"
  set -e
}

wggen_case w10_50_3        10 50 3
wggen_case w30_300_5       30 300 5
wggen_case w200_65_1       200 65 1
wggen_case w60_150_3       60 150 3
wggen_case w2_100_9        2 100 9
wggen_case w1_50_1         1 50 1
wggen_case err_seed0       10 50 0
wggen_case err_vertices    0 5 1
wggen_case err_weight      10 30000 1
wggen_case err_usage       5 5

# ---- weighted graphs, and wsub_search: name | graph | parameters | fixed vertices
wgraph() { "$bin/wggen" "${@:2}" | grep -- '-1$' > "$out/graphs/$1.txt"; }
wgraph w200_65_1  200 65 1
wgraph w60_150_3  60 150 3
wgraph w100_40_7  100 40 7

wsub_case() {
  local name=$1 g=$2 params=$3 fixed=${4:-}
  { echo "$params"; cat "$out/graphs/$g.txt"; [[ -n $fixed ]] && echo "$fixed"; } > "$out/wsub_search/$name.in"
  set +e
  "$bin/wsub_search" < "$out/wsub_search/$name.in" > "$out/wsub_search/$name.out" 2> "$out/wsub_search/$name.err"
  echo $? > "$out/wsub_search/$name.status"
  set -e
}

wsub_case light          w200_65_1  "200 10 30 0 20 1000 200 5 3 1"
wsub_case light_adj      w200_65_1  "200 10 30 0 10 1000 200 5 3 -2"
wsub_case heavy          w200_65_1  "200 15 120 0 10 3000 300 10 8 2"
wsub_case multi_weights  w60_150_3  "60 8 60 0 20 1500 250 6 21 -3"
wsub_case zero_weight    w60_150_3  "60 6 0 0 20 1500 250 6 4 1"
wsub_case fixed          w100_40_7  "100 12 25 2 15 1500 300 8 13 -1" "5 77"
wsub_case unit_weights   exp_100_600_1 "100 11 55 0 5 100 25 4 72 1"
wsub_case unit_hard      exp_100_600_1 "100 12 66 0 4 2000 300 8 5 -2"
wsub_case hard_180       w200_65_1  "200 15 180 0 20 3000 300 10 5 1"
wsub_case hard_183       w200_65_1  "200 15 183 0 10 3000 300 10 6 -1"
wsub_case multi_border   w60_150_3  "60 8 90 0 20 3000 300 10 5 3"
wsub_case unreachable    w100_40_7  "100 6 900 0 3 50 10 2 3 1"
wsub_case zero_moves     w100_40_7  "100 6 9 0 3 0 10 2 3 1"
wsub_case err_size       w100_40_7  "100 1 0 0 3 50 10 2 3 1"

# ---- reg_search and ereg_search (same input): name | graph | parameters | fixed vertices
# The Paley graph on q vertices (q prime, q = 1 mod 4) is strongly regular, the kind of graph
# reg_search was written for: i ~ j iff i - j is a non-zero square mod q.
paley() {
  python3 - "$1" > "$out/graphs/paley_$1.txt" <<'PY'
import sys
q = int(sys.argv[1])
squares = {x * x % q for x in range(1, q)}
for i in range(q):
    print("".join(f" {j}" for j in range(i + 1, q) if (j - i) % q in squares) + "-1")
PY
}
paley 61

reg_case() {
  local name=$1 g=$2 params=$3 fixed=${4:-}
  for prog in reg_search ereg_search; do
    { echo "$params"; cat "$out/graphs/$g.txt"; [[ -n $fixed ]] && echo "$fixed"; } > "$out/$prog/$name.in"
    set +e
    "$bin/$prog" < "$out/$prog/$name.in" > "$out/$prog/$name.out" 2> "$out/$prog/$name.err"
    echo $? > "$out/$prog/$name.status"
    set -e
  done
}

reg_case doc_4cycles     exp_100_600_1  "100 4 2 1 100 500 150 2 7 0" "0"
reg_case cycles          exp_300_140_7  "300 8 2 0 20 1000 200 5 3 1"
reg_case cubic           exp_300_140_7  "300 10 3 0 20 2000 300 8 5 -2"
reg_case independent     exp_300_140_7  "300 12 0 0 20 1000 200 5 9 1"
reg_case near_clique     exp_100_600_1  "100 8 7 0 20 1000 200 5 11 1"
reg_case paley_4reg      paley_61       "61 10 4 0 20 2000 300 8 13 -2"
reg_case paley_6reg      paley_61       "61 14 6 0 15 3000 300 10 17 -3"
reg_case paley_fixed     paley_61       "61 9 4 2 15 2000 300 8 19 2" "0 1"
reg_case dense_5reg      exp_100_600_2  "100 12 5 0 15 3000 300 10 23 -4"
reg_case geometric       geo_400_150_1  "400 10 3 0 15 2000 300 8 29 3"
reg_case unreachable     exp_30_300_5   "30 6 9 0 3 50 10 2 3 1"
reg_case zero_moves      exp_30_300_5   "30 6 2 0 3 0 10 2 3 1"
reg_case err_size        exp_30_300_5   "30 30 2 0 3 50 10 2 3 1"
reg_case err_freq        exp_30_300_5   "30 6 2 0 3 50 0 2 3 1"

echo "fixtures written to $out"
