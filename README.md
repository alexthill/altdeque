An Alernative Deque implemention written in Rust.

AltDeque is an alternative to the standard library's `VecDeque`. It exposes
mostly the same methods and has the performance characteristics. But
instead of using a ring buffer to achieve efficient insertion on both
ends, it uses two stacks. One stack to `push` and `pop` elements for each
end of the deuque. If `pop` is called on one end but it's stack is empty, then all
elements from the other stack are removed, reversed and put into it. This
operation takes *O(n)* time (where n is the length of the deque) but
after it *n* elements can be popped in constant time resulting in an
amortized runtime of *O(1)* for popping.

For more efficient memory usage both stacks are located at the ends of one
allocated buffer:
```
        growth ->               <- growth
+- back stack --+               +- front stack -+
|               |               |               |
v               v               v               v
+---+---+---+---+---+---+---+---+---+---+---+---+
| 4 | 5 | 6 | 7 |   |   |   |   | 0 | 1 | 2 | 3 |
+---+---+---+---+---+---+---+---+---+---+---+---+
                  |               |
            head -+               +- tail
```

This stack based approach has some advantages over a ringbuffer:
- no need for masks or modular arithmetic to access elements
- no need for a power of 2 capacity
- no need to always leave one element empty

But it also has a few disadvantges:
- accessing elemnts needs an additional branch to check in which stack they are
- popping elements is only *amortized* constant time, a single pop-call will
  take linear time if the coresponding stack is empty
- popping elements alternating from both sides is very inefficient as all
  elements need to be moved from one side to the other every time the side is changed

In my simple tests `AltDeque` and `VecDeque` are about equally fast for a simply
`push_back` and `pop_front` workload.


Some of the code and a lot of the docs and examples are taken from the code in the
[rust repository](https://github.com/rust-lang/rust/), so credits to it's contributors.
