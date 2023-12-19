
## Polycubes

Python and Rust implementations of a hashtable-less polycube enumerator using the method described by presseyt (see link below).  The python implementation can run on multiple CPU cores and can be halted and resumed.

This can theoretically be run across multiple machines (sum their final counts to find the final answer) even by hand after splitting its written `.json.gz` file into multiple files.  (It's not worth pursuing this though with the python implementation!)

### Running

```
python cubes.py --threads 4 -n 11
```

To halt (and save progress to a file):

```
touch halt-signal.txt
```

To resume from saved file:

```
python cubes.py --threads 4 --resume-from-file halt-n11-20231209T075457.json.gz
```

### File Size

With the default `--spawn-n 8`, 6922 polycubes are used for spawning threads.  If all of those 6922 are all written to disk for resuming later, the file will be about 28kb in size.

With `--spawn-n 10`, 346543 polycubes are used for spawning threads.  If all of those 346543 are all written to disk for resuming later, the file will be about 1.4mb in size.

### Background

See https://github.com/mikepound/opencubes for the original implementation and community-provided improvements from the Computerphile video at https://www.youtube.com/watch?v=g9n0a0644B4

Uses a method defined by GitHub user "presseyt" to find a "canonical" representation of a cube.  The "canonical" representation of a cube is rotation-independent.

This method doesn't require a hashtable to store all seen unique polycubes: when we try to add a new `n+1`th cube to a polycube of size `n`, we only count it as a unique polycube if removing the "least significant" cube (according to its new canonical representation) from that `n+1` cube leaves us with the canonical cube of size `n`.

When we find a new canonical polycube of size `n+1`, we proceed and evaluate that new larger polycube and all its larger decendants.  By giving a set of threads/cores/machines their own separate list of polycubes, they can independently count their polycubes' decendant shapes without exchanging any information with each other.  At the end of computation, we sum all their findings to find the number of unique polycubes of the target size.

Much of this code is a python 3 port of their javascript implementation, but there are a few changes, including:

- using a single integer to represent the encoding of the polycube's adjacency graph
- using a recursive depth-first traversal to build the adjacency graph
- using the full 6 bits for each cube in the adjacency graph (not truncating once all cubes are represented)
- a `Process` implementation that divides the work among multiple processes (CPU cores)

See the original javascript implementation and README.md file at:

https://github.com/mikepound/opencubes/tree/9ad224fd4f35b31d5b37d62fcbd1b871f9b9600c/javascript

### Python Running times

Running times (in seconds) for `--threads 0 -n 11` on an M1 Mac mini:
| cpython |  pypy |  commit | note  |
|   ---: |  ---: |  :---:  | :---: |
| 1050.0 |       | 44f96a5 |       |
|  768.5 | 284.0 | cb8a167 | copy the Polycube instances less often |
|  752.4 | 275.5 | df2c5a0 |       |
|  736.6 | 274.3 | 253a957 |       |
|  522.3 | 172.9 | f39b559 | abandon inferior encodings early |
|        | 163.7 | 0c511c4 | faster tracking for when try_pos is least significant |
| *221.2 | *63.6 |         | *attempt snowmanam2 algo |

**\*** _my best attempt at snowmanam2's algorithm is very fast, but it uses lots of memory and is not perfectly parallel so it wouldn't be useful at this point for trying to enumerate for large values of n_

Running times (in seconds) for `--threads 7 --spawn-n 7 -n 11` on an M1 Mac mini:
| cpython |  pypy |  commit | note  |
|   ---: |  ---: |  :---:  | :---: |
| 830.8  | 638.8 | cb8a167 |       |
|        | 702.0 | 1d0b809 |       |
| 914.7  | 671.5 | 6273cbd |       |
| 758.6  | 306.0 | d1a6a62 |       |
| 175.0  |  73.5 | 00ab2cf | Use ProcessPoolExecutor to acutally run on >1 CPU |
| 171.4  |  70.5 | 253a957 |       |
| 309.1  | 170.4 | 9f94cd2 | graceful halt refactor |
| 188.8  |  90.3 | 59ec0e5 | move `Pipe.poll()` out of loops |
| 138.3  |  72.0 | f39b559 | abandon inferior encodings early |
| 123.7  |  55.0 | 052f064 | probabilistic halt check |
| 113.2  |  50.1 | 633ba18 | don't re-encode when try_pos==least significant |
| 128.9  |  49.3 | 1f41545 | Workers use one Polycube instance |
| 117.0  |  45.8 | 0c511c4 | faster tracking for when try_pos is least significant |

### Rust Running Times

<!-- cargo +nightly-aarch64-apple-darwin build --release -->
<!-- cargo   +stable-x86_64-apple-darwin build --release -->
<!-- ./target/release/cubes-rust --threads 0 -n 11       -->

Running times (in seconds) for `--threads 0 -n 11` on an M1 Mac mini:
| x86_64 | aarch64 |  commit | note  |
|   ---: |    ---: |  :---:  | :---: |
|  281.1 |   152.7 | 792714c | ported from python |
|  207.2 |   109.2 | d58f958 | changed HashMap to BTreeMap |
|  157.3 |    86.5 | 65a1e61 | changed HashSet to BTreeSet |
|  152.6 |    83.7 | 1831859 | use one BTreeSet for enc and neighbors |
|  139.9 |    73.9 | 2bb422e | don't re-encode when try_pos==least significant |
|   58.7 |    42.5 | 5c98245 | faster tracking for when try_pos is least significant |

Running times (in seconds) for `--threads 7 --spawn-n 7 -n 11` on an M1 Mac mini:
| x86_64 | aarch64 |  commit | note  |
|   ---: |    ---: |  :---:  | :---: |
|        |    27.1 | 2bc8c1d | ported multi-thread from python |
|   33.1 |    22.8 | 2bb422e | don't re-encode when try_pos==least significant |
|   13.5 |    10.3 | 5c98245 | faster tracking for when try_pos is least significant |
|   13.5 |    10.3 | 5525df3 | Workers use one Polycube instance |

Running times (in seconds) for `--threads 7 --spawn-n 8 -n 12` on an M1 Mac mini:
| x86_64 | aarch64 |  commit | note  |
|  122.4 |    90.3 | 5525df3 | Workers use one Polycube instance |
