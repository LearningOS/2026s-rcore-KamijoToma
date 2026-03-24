# Lab 3 Report

FYI: 程序本身由codex生成，由人类阅读并填写了这一部分

这一章的要点在于：

1. 移植 ch4 代码到 ch5
2. 处理 spawn。相较于fork+exec，spawn取消了完整复制父进程地址空间的做法，直接从 ELF 文件生成新进程
3. stride 调度算法

伪代码：
```
TARGET_PROCESS = MIN(PROCESS_LIST[].stride)
TARGET_PROCESS.stride += MAX(1, BIG_STRIDE/TARGET_PROCESS.priority)
```
