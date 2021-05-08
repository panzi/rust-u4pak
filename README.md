Rust u4pak
==========

**Work in progress**

More or less re-implementation of [Python u4pak](https://github.com/panzi/u4pak)
for fun, ease of use (standalone binary), and hopefully speed.

Note that checking archive validity is acutally a bit faster in the Python
version since Python's SHA-1 implementation is faster. (Tell me if you know a
fater SHA-1 implementation for Rust than the one in `rust-crypto`.)

TODO
----

* [x] info
* [x] list
* [x] check
* [x] unpack
* [x] pack
* [x] multithreading
* [ ] maybe read arguments from text file for Windows users that can't handle a terminal?
* [ ] mount
