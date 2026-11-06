# Lab3（ch5）实验报告

## 实现的功能

先把上一章的 sys_get_time、sys_mmap、sys_munmap 迁移到新的进程结构上。get_time 改用微秒换算成秒和微秒，再按页翻译用户地址写回，处理了跨页的情况；mmap/munmap 保留了对齐、权限和区间检查。新增 sys_spawn，直接用 ELF 新建 TCB，不复制父进程地址空间，设置好父子关系后加入就绪队列。实现了 stride 调度：TCB 里加了 stride、pass、prio，BIG_STRIDE 是常量，调度时遍历就绪队列取 stride 最小的任务，被选中后累加 pass。sys_set_priority 要求 prio ≥ 2。本地 ch5 测试 15/15 通过。

## 问答题

**1. 8 bit 存 stride，p1.stride = 255，p2.stride = 250，pass 都是 10，p2 跑完一个时间片后真的轮到 p1 吗？**

不会。p2 加完 pass 是 260，8 位无符号溢出后变成 4。这时比较 4 < 255，调度器认为 p2 的 stride 更小，又选了 p2，p1 反而一直等着。

**2. 为什么优先级都 ≥ 2 时，STRIDE_MAX – STRIDE_MIN <= BigStride / 2？**

优先级 ≥ 2，所以每个进程的 pass = BigStride / prio ≤ BigStride / 2。每次被调度的都是 stride 最小的那个，它加上一个 pass 后，最多比原来的最小值大 BigStride / 2。用归纳的思路看：一开始都是 0，差值为 0；假设某一时刻所有 stride 都落在 [MIN, MIN + BigStride/2] 里，选中 MIN 加 pass 后新值不超过 MIN + BigStride/2，而新的最小值只会 ≥ MIN，所以最大值和最小值的差仍然不超过 BigStride / 2。

**3. 考虑溢出的 partial_cmp**

既然真实差值不会超过 BigStride / 2，就可以看两个数相减（按溢出回绕）的结果：如果回绕后的差落在一半范围以内，说明 self 实际更大，否则说明 self 更小。用有符号数来看这个差最直接：

```rust
use core::cmp::Ordering;

struct Stride(u64);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // 差值按回绕计算后当成有符号数，正数说明 self 在 other 后面
        let diff = self.0.wrapping_sub(other.0) as i64;
        if diff > 0 {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Less)
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}
```

前提是 BigStride 不超过 u64 能表示范围的一半（也就是差值能放进 i64 的正数范围），这样才能保证回绕后的符号判断是对的。另外 BinaryHeap 是大顶堆，实际用的时候要用 `Reverse` 包一下，或者把上面的结果反过来，pop 才能拿到最小的 stride。

## 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 **以下各位** 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

   > 与 AI 助手讨论过调试问题：编译错误、UPSafeCell 重复借用、get_time 单位换算、找最小 stride 时的初值等（只讨论问题所在，代码由我自己编写）

2. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

   > rCore-Tutorial-Guide 2026 文档（第五章及练习说明）；Rust 官方文档（`alloc::collections::VecDeque` 等）

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 对本次实验的看法

工作量主要在迁移上一章的代码，结构变了之后很多地方要跟着改，比想象中花时间。stride 本身不难，但踩了几个坑：get_time 拿的是毫秒却按微秒换算，导致 sleep 一直出不来；在 task_inner 还没释放的时候又调用了要借 inner 的函数，UPSafeCell 直接 panic；找最小 stride 时初始值写成了一个固定小数，stride 涨上去以后就退化成轮转了。这些错误现象都不太直观，排查花了不少时间。另外在 macOS 上用 docker 挂载目录编译，偶尔会遇到构建缓存的问题，改了代码没生效，希望文档里能提一下。
