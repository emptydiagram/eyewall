# eyewall

(WIP) App to crawl the bluesky post graph and find outlier graph components.

There are three post graphs:

 - reply-only
 - quote-only
 - reply-quote

We want to find weakly-connected components in all three graphs with:

 - a large number of nodes
 - a large depth (max path length from source to sink)
 - a large width (max size of an "antichain", or equivalently by [Dilworth's theorem](https://en.wikipedia.org/wiki/Dilworth%27s_theorem) the min number of chains needed to cover the subgraph)

and maybe some other statistics.


This allows you to find the largest/longest/widest reply trees, quote trees, and reply-quote subgraphs.


## Limitations

Only knows about posts that are related via post graph, and nothing about posts that are semantically related but not graph-ically related (i.e. it doesn't see "subtweets").
