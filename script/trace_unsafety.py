import fileinput
import re
import sys
import logging
import os
import sys

# Add vendor directory to module search path
parent_dir = os.path.abspath(os.path.dirname(__file__))
vendor_dir = os.path.join(parent_dir, 'vendor')

sys.path.append(vendor_dir)

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

# Returns whether the given name is among the things that have tainted this node
def is_tainted_by(graph, node, name):
    return name in graph.nodes[node].get("tainted_by", set())

# Adds the given name to the list of things that have touched this node
def taint_node(graph, node, name):
    tainted_by = graph.nodes[node].get("tainted_by", set())
    tainted_by.add(name)
    graph.nodes[node]["tainted_by"] = tainted_by

# Propagates a taint through a graph breadth-first
def propagate_taint(graph, start_node):
    start_label = graph.nodes[start_node].get("label")
    log.info(f"taint starting at {start_label}")

    # We mark all the nodes touched in this function call and then increment all their badnesses by
    # 1 at the very end. This way we don't double-count nodes in cycles. This function is never
    # called with the same start_node twice, so the name used for tainting here is unique.
    all_nodes_touched = {start_node}
    taint_node(graph, start_node, start_node)

    # Initialize the first generation in the breadth-first search
    this_gen = set(filter(
        lambda n: not is_tainted_by(graph, n, start_node),
        graph.predecessors(start_node),
    ))
    all_nodes_touched.update(this_gen)

    # We update this_gen every iteration. Any node that is a predecessor of a this_gen node that has
    # not yet been tainted in this function will be included in the next generation
    while True:
        # Add this generation to the list of all nodes we tainted so far
        all_nodes_touched.update(this_gen)

        # No more nodes left to taint
        if len(this_gen) == 0:
            break

        # Process this generation. Taint the nodes and accumulate their untainted parents. These
        # constitute the next generation.
        next_gen = set()
        for node in this_gen:
            taint_node(graph, node, start_node)
            log.info("tainting {}".format(graph.nodes[node]["label"]))

            # Find the adjacent nodes in the callgraph that we haven't seen yet. This is the next
            # generation of nodes to taint.
            untouched_callers = filter(
                lambda n: not is_tainted_by(graph, n, start_node),
                graph.predecessors(node),
            )
            next_gen.update(untouched_callers)

        this_gen = next_gen.copy()


    # Increment everyone's badnesses
    for node in all_nodes_touched:
        graph.nodes[node]["badness"] += 1

# Given a graph, returns the subgraph of nodes that have a nonzero badness
def tainted_nodes(graph):
    return filter(lambda n: graph.nodes[n].get("badness", 0) > 0, iter(graph))

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
            # See if this is a node we should taint
            if label in node_labels_to_taint:
                log.debug(f"found a node we want to taint: {n}")
                node_ids_to_taint.add(n)

        for n in node_ids_to_taint:
            propagate_taint(graph, n)

        sg = tainted_nodes(graph)

        # To print this out, we have to dedup all the node labels, since multiple nodes can have the
        # same label
        label_to_badness = dict()
        for n in sg:
            label = graph.nodes[n]["label"]
            tot_occurrence = graph.nodes[n]["badness"] + label_to_badness.get(label, 0)
            label_to_badness[label] = tot_occurrence

        # Sort by badness in descending order
        sorted_pairs = sorted(label_to_badness.items(), key=lambda kv: kv[1], reverse=True)

        print("Badness  Function")
        for (label, badness) in sorted_pairs:
            # Match `CRATENAME::` preceded by any number of open angled brackets
            if re.match(r"^<*{}::".format(filter_prefix), label):
                print("    {:03}  {}".format(badness, label))

if __name__ == "__main__":
    try:
        main()
    except IndexError:
        print("USAGE:")
        print(f"{sys.argv[0]} [GRAPH_FILE] [TAINT_FILE] [FILTER_PREFIX]")
