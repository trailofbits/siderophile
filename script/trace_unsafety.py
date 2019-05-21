import fileinput
import re
import sys
import logging
import os

import networkx as nx

log = logging.getLogger()
logging.basicConfig(level=os.environ.get("LOGLEVEL", "ERROR"))

# Mostly taken from the Praezi README
def load_callgraph(f):
    callgraph = nx.DiGraph()
    for line in f:
        if "->" not in line:
            g = re.match(r'^\W*(.*?) \[shape=record,label="{(.*?)}"\];', line)
            if g:
                node_id = g.group(1)
                label = g.group(2)
                callgraph.add_node(node_id, label=label, badness=0)
            else:
                pass # This happens on lines that aren't nodes or edges
        else:
            g = re.match('\W*(.*) -> (.*);', line)
            if g:
                from_node_id = g.group(1)
                to_node_id = g.group(2)
                callgraph.add_edge(g.group(1), g.group(2))
            else:
                pass # This happens on lines that aren't nodes or edges
    return callgraph

def is_tainted(graph, node):
    return graph.nodes[node].get("badness", 0) > 0

def propagate_taint(graph, start_node):
    start_label = graph.nodes[start_node].get("label")
    log.info(f"tainting {start_label}")

    # We mark all the nodes touched in this pass and then increment all their badnesses by 1 at the
    # very end. This way we don't double-count nodes in cycles.
    all_nodes_touched = set([start_node])

    this_gen = list(filter(lambda n: not is_tainted(graph, n), graph.predecessors(start_node)))
    all_nodes_touched.update(set(this_gen))

    while True:
        # Add this generation to the list of all nodes we tainted so far
        all_nodes_touched.update(set(this_gen))

        if len(this_gen) == 0:
            break

        next_gen = []
        for node in this_gen:
            label = graph.nodes[node]["label"]
            log.info(f"tainting {label}")

            # Find the adjacent nodes in the callgraph that we haven't seen yet. This is the next
            # generation of nodes to taint.
            untouched_callers = filter(
                lambda n: n not in all_nodes_touched,
                graph.predecessors(node),
            )
            next_gen.extend(list(untouched_callers))

        this_gen = next_gen[:]

    # Increment their badnesses
    for node in all_nodes_touched:
        graph.nodes[node]["badness"] += 1

# Given a graph, return the subgraph of nodes that have a nonzero badness
def tainted_subgraph(graph):
    tainted_nodes = list(filter(lambda n: is_tainted(graph, n), iter(graph)))
    return graph.subgraph(tainted_nodes)

def main():
    filter_prefix = sys.argv[3]

    with open(sys.argv[1], "r", encoding="utf-8") as graph_file,\
         open(sys.argv[2], "r", encoding="utf-8") as taint_file:
        graph = load_callgraph(graph_file)

        # Read in the labels to taint. Any line beginning with a '#' is ignored
        node_labels_to_taint = set(filter(
            lambda line: not line.startswith("#"),
            taint_file.read().splitlines(),
        ))

        log.debug(f"node_labels_to_taint == {node_labels_to_taint}")

        node_ids_to_taint = set()
        for n in iter(graph):
            # Somehow we found a node without a label. Skip it
            if "label" not in graph.nodes[n]:
                continue

            label = graph.nodes[n]["label"]
            # See if this is a node we shoudl taint
            if label in node_labels_to_taint:
                log.debug(f"found a node we want to taint: {n}")
                node_ids_to_taint.add(n)

        for n in node_ids_to_taint:
            propagate_taint(graph, n)

        sg = tainted_subgraph(graph)

        # To print this out, we have to dedup all the node labels, since multiple nodes can have the
        # same label
        label_to_badness = dict()
        for n in list(iter(sg)):
            label = sg.nodes[n]["label"]
            tot_occurrence = sg.node[n]["badness"] + label_to_badness.get(label, 0)
            label_to_badness[label] = tot_occurrence

        # Sort by badness in descending order
        sorted_pairs = sorted(label_to_badness.items(), key=lambda kv: kv[1], reverse=True)

        print("Badness  Function")
        for (label, badness) in sorted_pairs:
            if re.match(r"[^:]{}".format(filter_prefix), label):
                print("    {:03}  {}".format(badness, label))

if __name__ == "__main__":
    try:
        main()
    except IndexError:
        print("USAGE:")
        print(f"{sys.argv[0]} [GRAPH_FILE] [TAINT_FILE] [FILTER_PREFIX]")
