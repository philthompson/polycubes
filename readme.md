
## Polycubes

A python 3 implementation of a hashtable-less polycube enumerator.

## Background

See https://github.com/mikepound/opencubes for the original implementation and community-provided improvements from the Computerphile video at https://www.youtube.com/watch?v=g9n0a0644B4

Uses a method defined by GitHub user "presseyt" to find a "canonical" representation of a cube.

The "canonical" reporesentation of a cube is rotation-independent.

This method avoids using a hashtable to store all seen unique polycubes: when we try to add a new `n+1`th cube to a polycube, we only count that as a unique polycube of size `n+1` if removing the "least significant" cube from that `n+1` cube leaves us with the canonical cube of size `n`.

Much of this code is a python 3 port of their javascript implementation, but there are a few changes, including:

- using a single integer to represent the encoding of the polycube's adjacency graph
- using a recursive depth-first traversal to build the adjacency graph
- using the full 6 bits for each cube in the adjacency graph (not truncating once all cubes are represented)
- a ProcessPoolExecutor implementation that divides the work among multiple processes (threads)

See the original javascript implementation and README.md file at:

https://github.com/mikepound/opencubes/tree/9ad224fd4f35b31d5b37d62fcbd1b871f9b9600c/javascript

## Running times

Running times (in seconds) for single-threaded, **n=11**, on an M1 Mac mini:
| python |  pypy |  commit |
|   ---: |  ---: |  :---:  |
| 1050.0 |       | 44f96a5 |
|  768.5 | 284.0 | cb8a167 |
|  752.4 | 275.5 | df2c5a0 |
|  736.6 |       | 253a957 |

Running times (in seconds) for 7 threads, **n=11**, on an M1 Mac mini:
| python |  pypy |  commit |
|   ---: |  ---: |  :---:  |
| 830.8  | 638.8 | cb8a167 |
|        | 702.0 | 1d0b809 |
| 914.7  | 671.5 | 6273cbd |
| 758.6  | 306.0 | d1a6a62 |
| 175.0  |  73.5 | 00ab2cf |
| 171.4  |  70.5 | 253a957 |
