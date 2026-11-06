# rCore Chapter 3 实验报告

## 一、功能总结

本次实验主要在第三章分时多任务系统的基础上实现 `sys_trace` 系统调用。我在每个任务的 `TaskControlBlock` 中增加系统调用计数数组，用于分别记录各任务调用不同系统调用的次数，并在统一的 `syscall` 入口进行统计。`sys_trace` 实现了读取一个字节、写入一个字节以及查询当前任务指定系统调用次数三个功能。最终通过 ch3 全部 7 项测试。

---

## 二、实验实现

### 1. 系统调用次数统计

题目要求查询的是当前任务自身的系统调用次数，因此我把统计数据放到了每个任务的 `TaskControlBlock` 中：

```rust
syscall_times: [u32; 512]
```

数组的下标表示系统调用号，对应位置保存该系统调用已经执行的次数。

例如：

```text
syscall_times[64]
```

表示当前任务调用编号为 64 的系统调用的次数。

然后在统一的 `syscall` 函数中，在真正进行系统调用分发之前进行统计：

```rust
record_syscall(syscall_id);
```

这样所有系统调用都会被记录。

这里需要注意，`sys_trace` 本身也属于系统调用，所以查询 `trace` 的调用次数时，本次 `trace` 也必须已经被统计，因此记录操作要放在 `match` 分发之前。

### 2. sys_trace

`sys_trace` 根据 `trace_request` 分成三种情况。

当：

```text
trace_request = 0
```

时，把 `id` 当成一个指针，读取这个地址处的一个字节并返回：

```rust
*(id as *const u8)
```

当：

```text
trace_request = 1
```

时，把 `id` 当成可写地址，将 `data` 的低 8 位写入这个地址：

```rust
*(id as *mut u8) = data as u8;
```

当：

```text
trace_request = 2
```

时，把 `id` 当成系统调用号，从当前任务的 `syscall_times` 中取出对应的调用次数。

因为第三章还没有实现独立地址空间和完整的用户内存检查，所以读写用户地址时直接使用了裸指针和 `unsafe`。

---

## 三、简答题

### 1. 正确进入 U 态后执行 S 态特权操作会发生什么？

用户程序运行在 U 态时，不能直接执行只允许 S 态执行的特权指令，也不能随意访问 S 态的寄存器。

运行三个 bad 测例后，可以看到非法访问地址会触发 PageFault，而执行不允许的指令或访问对应寄存器会出现 IllegalInstruction。之后内核会进入 Trap 处理，并结束出错的应用。

本实验使用：

```text
RustSBI version 0.4.1
```

---

### 2. 刚进入 `__restore` 时，`sp` 表示什么？`__restore` 有哪两种使用情况？

`__restore` 的作用是恢复之前保存的 `TrapContext`，最后返回用户态。

进入 `__restore` 时还处在内核态，此时需要根据 `TrapContext` 恢复用户程序之前的寄存器状态。

我理解 `__restore` 主要有两种使用情况。

第一种是应用第一次运行。

任务第一次被调度时，本身并没有“以前运行到一半留下来的上下文”，所以内核会提前构造一个 `TrapContext`，同时让任务的 `TaskContext.ra` 指向 `__restore`。第一次进行任务切换之后，通过 `ret` 进入 `__restore`，恢复事先准备好的用户上下文，最后进入用户态。

第二种是处理完 Trap 后返回用户程序。

用户程序执行 `ecall` 或发生其他 Trap 后，会先进入内核处理。处理结束后，再通过 `__restore` 恢复之前保存的用户寄存器，然后回到用户态继续执行。

---

### 3. 下面几行特殊处理了哪些寄存器？有什么作用？

```asm
ld t0, 32*8(sp)
ld t1, 33*8(sp)
ld t2, 2*8(sp)
csrw sstatus, t0
csrw sepc, t1
csrw sscratch, t2
```

这里主要恢复三个比较特殊的内容：

```text
sstatus
sepc
用户态 sp
```

`sepc` 决定执行 `sret` 以后从用户程序的哪个地址继续执行。

`sstatus` 保存处理器的状态，其中的相关位决定 `sret` 后返回哪个特权级。

`TrapContext.x[2]` 中保存的是用户程序原来的 `sp`。在内核中运行时需要使用内核栈，所以用户栈指针要暂时保存，返回用户态之前再进行恢复。

可以简单理解成：

```text
sepc      -> 返回到哪里
sstatus   -> 返回到什么状态/特权级
sp        -> 返回以后使用哪个栈
```

---

### 4. 为什么恢复通用寄存器的时候跳过 `x2` 和 `x4`？

`x2` 就是 `sp`。

Trap 处理期间，当前的 `sp` 正在用来访问内核栈上的 `TrapContext`，所以不能像普通寄存器一样提前恢复。

用户程序原来的 `sp` 被单独保存，在最后通过 `sscratch` 和 `sp` 的交换恢复。

`x4` 是 `tp`。在目前的实验环境中没有依赖用户态 `tp` 保存需要恢复的执行状态，因此这里没有像其他普通寄存器一样处理。

---

### 5. `__restore` 中执行 `csrrw sp, sscratch, sp` 后，两个寄存器分别表示什么？

执行之前可以理解为：

```text
sp        -> 内核栈
sscratch  -> 用户栈指针
```

执行：

```asm
csrrw sp, sscratch, sp
```

后，两者的值发生交换：

```text
sp        -> 用户栈
sscratch  -> 原来的内核栈指针
```

因此接下来进入用户态以后，用户程序就重新使用自己的用户栈。

---

### 6. `__restore` 中在哪条指令真正完成特权级切换？

真正从 S 态返回 U 态的是：

```asm
sret
```

在执行 `sret` 之前，内核已经恢复了 `sstatus` 和 `sepc`。

其中 `sstatus` 中保存了返回后的特权级信息，`sepc` 保存返回后继续执行的位置。

因此执行 `sret` 后，CPU 根据这些信息恢复执行状态并进入用户态。

---

### 7. `__alltraps` 中执行 `csrrw sp, sscratch, sp` 后是什么意思？

用户程序正常在 U 态运行时：

```text
sp        -> 用户栈
sscratch  -> 内核栈位置
```

发生 Trap 进入内核后：

```asm
csrrw sp, sscratch, sp
```

交换二者，变成：

```text
sp        -> 内核栈
sscratch  -> 用户程序原来的 sp
```

这样后面的 `__alltraps` 就可以直接使用内核栈，在上面保存 `TrapContext`。

而用户原来的栈指针暂时保存在 `sscratch` 中，后面再写入 `TrapContext.x[2]`。

---

### 8. 从 U 态进入 S 态是哪条指令发生的？

严格来说，U 态到 S 态的切换不是 `trap.S` 中某条普通汇编指令主动完成的，而是在发生 Trap 时由硬件完成。

例如系统调用中，用户程序执行：

```asm
ecall
```

之后产生 `Environment Call from U-mode` 异常。

处理器会保存 `sepc`、设置 `scause` 等状态，然后把当前特权级切换到 S 态，并根据 `stvec` 跳转到内核设置的 Trap 入口 `__alltraps`。

因此，对于系统调用来说，`ecall` 触发了这个过程，而实际的特权级切换由处理器的 Trap 机制完成。

---

## 四、实验中遇到的问题

这次实验前面比较容易把 `TaskContext` 和 `TrapContext` 搞混。

尤其是在看：

```asm
ld sp, 8(a1)
```

和：

```asm
sd t2, 2*8(sp)
```

的时候，一开始不理解为什么一个 `sp` 在偏移 8，一个又在偏移 16。

后来才发现它们操作的是两个完全不同的数据结构。

`TaskContext` 里面大致是：

```text
ra
sp
s0~s11
```

所以 `sp` 位于第二个 `usize`，也就是偏移 8。

而 `TrapContext` 中保存的是：

```text
x0
x1
x2
...
```

`x2` 本身就是 `sp`，所以它位于：

```text
2 * 8 = 16
```

另外，我一开始也不太理解：

```asm
ld sp, 8(a1)
```

为什么算任务切换的一部分。

后面理解到 `8(a1)` 保存的是下一个任务自己的内核栈指针。恢复这个值以后，CPU 的 `sp` 就切换到了下一个任务的内核栈。

因此可以理解为：

```text
ra -> 决定接下来从哪里继续执行
sp -> 决定接下来使用哪个任务的栈
```

在实现 `sys_trace` 的时候也遇到了一些 Rust 编译错误，比如：

```text
unused import
unused unsafe
cannot borrow as mutable
missing documentation
```

另外最开始系统调用计数使用的是：

```rust
[usize; 512]
```

后来改为了：

```rust
[u32; 512]
```

这样能够减少一些内存占用，读取时再转换成 `usize`。

---

## 五、实验结果

最后运行 Chapter 3 检查程序，所有测试均通过：

```text
[PASS] get_time
[PASS] Test sleep
[PASS] current time_msec
[PASS] sleep timing
[PASS] sleep1
[PASS] string from task trace test
[PASS] Test trace

Test passed: 7/7
```

说明本次实验的代码部分能够通过测试。

---

## 六、实验总结

通过这一章，我对任务切换、Trap 和系统调用之间的关系比之前清楚了很多。

目前我的理解是：

`TaskManager` 负责管理任务和选择下一个任务，`TaskControlBlock` 保存每个任务自己的状态，`TaskContext` 用来完成任务之间的切换，而 `TrapContext` 主要负责用户态和内核态之间切换时保存用户程序的完整执行现场。

任务切换的核心过程可以理解成：

```text
选择下一个任务
        ↓
保存当前 TaskContext
        ↓
恢复下一个 TaskContext
        ↓
切换 sp
        ↓
ret 到下一个任务
```

在多道程序系统中，任务可以主动通过 `yield` 让出 CPU。

加入定时器中断以后，即使任务不主动 `yield`，内核也能够通过时钟中断抢占当前任务，从而实现时间片轮转的分时多任务。

通过这次 `sys_trace` 实验，也把系统调用的过程进一步串起来了：

```text
用户程序
    ↓
ecall
    ↓
Trap
    ↓
__alltraps
    ↓
trap_handler
    ↓
syscall
    ↓
具体系统调用
    ↓
__restore
    ↓
sret
    ↓
用户程序
```

---

## 七、荣誉准则及参考情况

本次实验主要参考 rCore-Tutorial-Guide 第三章及 Chapter 3 实验说明。

实验过程中使用 ChatGPT 辅助理解和排查问题，主要涉及：

- `TaskManager`、`TaskControlBlock` 的作用；
- `TaskContext` 和 `TrapContext` 的区别；
- `switch.S` 和 `trap.S` 中部分汇编代码；
- `yield`、时钟中断和任务调度流程；
- `sys_trace` 的实现思路；
- Rust 编译错误的分析和修改。

实验代码经过本人阅读、理解、修改和调试，并在本地运行检查器，最终通过 Chapter 3 全部 7 项测试。

如课程荣誉准则要求使用固定格式，本部分在最终提交前按课程要求补充。