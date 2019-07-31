import re
import sys

# This funciton takes a Rust module path like
# `<T as failure::as_fail::AsFail>::as_fail and strips`
# down the fully-qualified trait paths within to just the base trait name, like
# `<T as AsFail>::as_fail`
def simplify_fqn_trait_paths(path):
    # Things that happen after " as " are what we care about
    parts = path.split(" as ")
    if len(parts) == 1:
        return path

    new_parts = []
    for (i, after_as) in enumerate(parts):
        # Every other segment here is what comes before the " as ", which we do not modify. So just
        # append it to the list and move on
        if i % 2 == 0:
            new_parts.append(after_as)
            continue

        # Read until the first ">" character, which marks the end of the trait path. We do not
        # modify *rest
        (path, *rest) = after_as.split(">")
        # This is the "AsFail" in the example at the top of this function
        basename = path.split("::")[-1]

        # Now add *rest back to the modified trait path
        new_after_as = ">".join([basename, *rest])
        new_parts.append(new_after_as)

    # Surgery complete. Stitch it all back up.
    return " as ".join(new_parts)

# A dumb unit test
def test_simplify_fqn_trait_paths():
    assert(
        simplify_fqn_trait_paths("<&mut std::collections::hash::table::RawTable<K,V> as std::collections::hash::table::Put<K,V>>::borrow_table_mut") ==
        "<&mut std::collections::hash::table::RawTable<K,V> as Put<K,V>>::borrow_table_mut"
    )
    assert(
        simplify_fqn_trait_paths("<futures::lock::TryLock<T> as core::ops::deref::Deref>::deref") ==
        "<futures::lock::TryLock<T> as Deref>::deref"
    )
    assert(
        simplify_fqn_trait_paths("<network::proto::state_synchronizer::RequestChunk as ::protobuf::Message>::default_instance") ==
        "<network::proto::state_synchronizer::RequestChunk as Message>::default_instance"
    )
    assert(
        simplify_fqn_trait_paths("<T as failure::as_fail::AsFail>::as_fail") ==
        "<T as AsFail>::as_fail"
    )

def main():
    with open(sys.argv[1], "r", encoding="utf-8") as graph_file,\
         open(sys.argv[2], "r", encoding="utf-8") as siderophile_out:

        # We map the simplified version of the graph labels to their fully-qualified version, i.e.,
        # the full label that appears in the callgraph. We want to do comparison of the former, but
        # the values we spit out have to be the latter.
        simplified_labels_to_fqn = dict()
        for line in graph_file:
            # We're looking for nodes, not edges
            if "->" not in line:
                g = re.match(r'^\W*.*? \[shape=record,label="{(.*?)}"\];', line)
                if g:
                    # The label is the item in the callgraph we care about
                    label = g.group(1)
                    simplified_label = simplify_fqn_trait_paths(label)

                    # In practice, very few nonequal paths simplify to the same path. So this
                    # assignment is not likely to overwrite anything. And if it does, whatevs
                    simplified_labels_to_fqn[simplified_label] = label

        # Use the simplified versions for all the siderophile outputs too
        siderophile_lines = set(map(
            simplify_fqn_trait_paths,
            siderophile_out.read().splitlines(),
        ))

        # Get the intersection of the simplified paths that occur in siderophile output and the
        # callgraph
        matches = siderophile_lines.intersection(set(simplified_labels_to_fqn.keys()))

        # Now print out the fully-qualified matches
        for simplified_match in matches:
            print(simplified_labels_to_fqn[simplified_match])

if __name__ == "__main__":
    # Run a unit test first
    test_simplify_fqn_trait_paths()
    try:
        main()
    except IndexError:
        print("USAGE:")
        print("{} [GRAPH_FILE] [SIDEROPHILE_OUTPUT]".format(sys.argv[0]))
