# Lab1 实验报告

## 环境配置

我是 QEMU8 环境，按照[第零章 实验环境配置](https://learningos.cn/rCore-Tutorial-Guide-2025S/0setup-devel-env.html#qemu)的“注意”小节配置SBI地址和最新版bootloader后即可正常运行测试。

## sys_trace 系统调用实现

### 读写实现

简单的裸指针读写即可

### 系统调用计数

在PCB中新增了一个Map实现

## 结果

通过了所有测试
