# ms_toollib

史上唯一专门的、跨平台、跨语言的扫雷算法工具箱。已发布到：

- crates.io
- pypi.org
- npmjs.com

可在Rust、Python(Windows、Linux)、Nodejs、VUE3/Typescript、C、C++、julia等语言/平台中使用，并提供案例，可快速上手。项目呈起步阶段，相关技术问题可以提供一对一交流解决。欢迎star、pull request、fork、issue（提需求、报bug等）

Algorithms for minesweeper, published on various platforms.

- 目前主要文档见[https://docs.rs/ms_toollib](https://docs.rs/ms_toollib)。
最新版本号统计：  
版本号越大，代表越新、功能越完善、bug越少（可以催）。

python>=3.7, <=3.10(windows): 1.4.1

python>=3.7, <=3.8(linux): 1.3.10

javascript/typescript: 1.2.10

nodejs: 1.2.7

crate: 1.4.2

C(仅windows): 1.0.0 （没有类，即没有录像解析工具、局面状态机等。调试环境为MSVC。Linux未经测试，但估计可用。没有包管理平台，需要用户用源码自行编译，目前需要安装rust工具链，自行编译得到.lib文件）

C++(仅Linux): 1.0.0
（采用Cmake构建、没有包管理平台，需要用户用源码自行编译，需要安装rust工具链。）


### 如何调试源码

在编译之前，请确保自己拥有：

*   c++开发工具(windows下尽量Visual Studio，即MSVC，依据操作系统、博客教程或安装rust环境过程中的提示)
*   Rust工具链(rustup -V能够打印版本号、cargo -V能够打印版本号)
*   Visual Studio Code及对应插件(例如rust-analyzer)
*   会用Powershell或者其它命令行工具的能力
*   安装完全部环境以后，还剩余至少6G的硬盘容量

以下为调试步骤：

*   克隆这个仓库到本地
```sh
    git clone https://github.com/eee555/ms_toollib.git
```

*   用Visual Studio Code打开base文件夹

*   编辑器打开需要执行的测试程序文件，例如tests/test_analyse.rs

*   找到您想要执行的测试程序，例如minesweeper_board_works这个函数，用鼠标点击#[test]下方的灰色的Run Test按钮，即可打印执行结果！
