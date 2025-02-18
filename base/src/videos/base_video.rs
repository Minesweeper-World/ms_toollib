// 录像相关的类，局面在board

use crate::board::GameBoard;
use crate::cal_cell_nums;
use crate::miscellaneous::{s_to_ms, time_ms_between};
use crate::utils::{cal_bbbv, cal_isl, cal_op, refresh_board};
use crate::videos::analyse_methods::{
    analyse_high_risk_guess, analyse_jump_judge, analyse_mouse_trace, analyse_needless_guess,
    analyse_super_fl_local, analyse_survive_poss, analyse_vision_transfer,
};
use std::cmp::{max, min};
use std::fs;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// 没有时间、像素观念的局面状态机，侧重分析操作与局面的交互、推衍局面。在线地统计左右双击次数、ce次数、左键、右键、双击、当前解决的3BV。  
/// - 局限：不关注具体的线路（没有像素观念），因此不能计算path等。  
/// - 注意：ce的计算与扫雷网是不同的，本工具箱中，重复标同一个雷只算一个ce，即反复标雷、取消标雷不算作ce。
/// 应用场景：强化学习训练AI、游戏复盘计算指标。不能处理高亮（18）、算法确定是雷（12）等标记。  
/// - 用python调用时的示例：
/// ```python
/// import ms_toollib as ms
/// board = [
///     [0, 0, 1, -1, 2, 1, 1, -1],
///     [0, 0, 2, 3, -1, 3, 3, 2],
///     [1, 1, 3, -1, 4, -1, -1, 2],
///     [2, -1, 4, -1, 3, 4, -1, 4],
///     [3, -1, 5, 2, 1, 3, -1, -1],
///     [3, -1, -1, 2, 1, 2, -1, 3],
///     [-1, 5, 4, -1, 1, 1, 2, 2],
///     [-1, 3, -1, 2, 1, 0, 1, -1],
///     ];
/// v = ms.MinesweeperBoard(board) # 实例化后再用
/// v.step('lc', (0, 0)) # 左键按下
/// v.step('lr', (0, 0)) # 左键弹起
/// print('左键次数: ', v.left)
/// print('右键次数: ', v.right)
/// print('ce数: ', v.ces)
/// print('标雷数: ', v.flag)
/// print('解决3BV数: ', v.solved3BV)
/// print('局面: ', v.game_board)
/// ```
pub struct MinesweeperBoard {
    pub board: Vec<Vec<i32>>,
    /// 局面
    pub game_board: Vec<Vec<i32>>,
    flagedList: Vec<(usize, usize)>, // 记录哪些雷曾经被标过，则再标这些雷不记为ce
    /// 左键数
    pub left: usize,
    /// 右键数
    pub right: usize,
    /// 双击数
    pub double: usize,
    /// ce数
    pub ce: usize,
    /// 标雷数
    pub flag: usize,
    /// 已解决的3BV数
    pub bbbv_solved: usize,
    pub row: usize,
    pub column: usize,
    pub mouse_state: MouseState,
    pub game_board_state: GameBoardState,
    // 指针，用于判断局面是否全部扫开
    pointer_x: usize,
    pointer_y: usize,
    pre_flag_num: usize,
    // 中键是否按下，配合“m”、“mc”、“mr”。
    middle_hold: bool,
}

impl Default for MinesweeperBoard {
    fn default() -> Self {
        MinesweeperBoard {
            board: vec![],
            game_board: vec![],
            flagedList: vec![],
            left: 0,
            right: 0,
            double: 0,
            ce: 0,
            flag: 0,
            bbbv_solved: 0,
            row: 0,
            column: 0,
            mouse_state: MouseState::Undefined,
            game_board_state: GameBoardState::Ready,
            pointer_x: 0,
            pointer_y: 0,
            pre_flag_num: 0,
            middle_hold: false,
        }
    }
}

impl MinesweeperBoard {
    pub fn new(board: Vec<Vec<i32>>) -> MinesweeperBoard {
        let row = board.len();
        let column = board[0].len();
        MinesweeperBoard {
            board,
            row,
            column,
            game_board: vec![vec![10; column]; row],
            flagedList: vec![],
            mouse_state: MouseState::UpUp,
            ..MinesweeperBoard::default()
        }
    }
    /// Playing状态下的左击，没有按下抬起之分
    fn left_click(&mut self, x: usize, y: usize) -> Result<u8, ()> {
        self.left += 1;
        if self.game_board[x][y] != 10 {
            return Ok(0);
        }
        refresh_board(&self.board, &mut self.game_board, vec![(x, y)]);
        match self.board[x][y] {
            0 => {
                self.bbbv_solved += 1;
                self.ce += 1;
                // refresh_board(&self.board, &mut self.game_board, vec![(x, y)]);
                if self.is_win() {
                    self.game_board_state = GameBoardState::Win;
                }
                Ok(2)
            }
            -1 => {
                // refresh_board(&self.board, &mut self.game_board, vec![(x, y)]);
                self.game_board_state = GameBoardState::Loss;
                Ok(0)
            }
            _ => {
                // refresh_board(&self.board, &mut self.game_board, vec![(x, y)]);
                if self.num_is_3BV(x, y) {
                    self.bbbv_solved += 1;
                }
                self.ce += 1;
                if self.is_win() {
                    self.game_board_state = GameBoardState::Win;
                }
                Ok(2)
            }
        }
    }
    /// Playing状态下的右击，没有按下抬起之分
    fn right_click(&mut self, x: usize, y: usize) -> Result<u8, ()> {
        self.right += 1;
        if self.game_board[x][y] < 10 {
            return Ok(0);
        } else {
            if self.board[x][y] != -1 {
                match self.game_board[x][y] {
                    10 => {
                        self.game_board[x][y] = 11;
                        self.flag += 1;
                    }
                    11 => {
                        self.game_board[x][y] = 10;
                        self.flag -= 1;
                    }
                    _ => return Err(()),
                }
            } else {
                match self.game_board[x][y] {
                    10 => {
                        self.game_board[x][y] = 11;
                        self.flag += 1;
                        if !self.flagedList.contains(&(x, y)) {
                            self.ce += 1;
                        }
                        self.flagedList.push((x, y));
                    }
                    11 => {
                        self.game_board[x][y] = 10;
                        self.flag -= 1;
                    }
                    _ => return Err(()),
                }
            }
            Ok(1)
        }
    }
    /// Playing状态下的双击，没有按下抬起之分
    fn chording_click(&mut self, x: usize, y: usize) -> Result<u8, ()> {
        self.double += 1;
        if self.game_board[x][y] == 0 || self.game_board[x][y] >= 8 {
            return Ok(0);
        }
        let mut flagChordingUseful = false; // 双击有效的基础上，周围是否有未打开的格子
        let mut chordingCells = vec![]; // 未打开的格子的集合
        let mut flagedNum = 0; // 双击点周围的标雷数
        let mut surround3BV = 0; // 周围的3BV
        let mut flag_ch_op = false; // 是否通过双击开空了：一次双击最多打开一个空
        for i in max(1, x) - 1..min(self.row, x + 2) {
            for j in max(1, y) - 1..min(self.column, y + 2) {
                if i != x || j != y {
                    if self.game_board[i][j] == 11 {
                        flagedNum += 1
                    }
                    if self.game_board[i][j] == 10 {
                        chordingCells.push((i, j));
                        flagChordingUseful = true;
                        if self.board[i][j] > 0 {
                            if self.num_is_3BV(i, j) {
                                surround3BV += 1;
                            }
                        } else if self.board[i][j] == 0 {
                            flag_ch_op = true;
                        }
                    }
                }
            }
        }
        if flagedNum == self.game_board[x][y] && flagChordingUseful {
            self.ce += 1;
            self.bbbv_solved += surround3BV;
            if flag_ch_op {
                self.bbbv_solved += 1;
            }
            for ch in &chordingCells {
                if self.board[ch.0][ch.1] == -1 {
                    self.game_board_state = GameBoardState::Loss;
                }
            }
            refresh_board(&self.board, &mut self.game_board, chordingCells);
            if self.is_win() {
                self.game_board_state = GameBoardState::Win;
            }
            Ok(3)
        } else {
            Ok(0)
        }
    }
    fn num_is_3BV(&self, x: usize, y: usize) -> bool {
        // 判断该大于0的数字是不是3BV
        // 如果是0，即使是3bv，依然返回false
        if self.board[x][y] <= 0 {
            return false;
        }
        for i in max(1, x) - 1..min(self.row, x + 2) {
            for j in max(1, y) - 1..min(self.column, y + 2) {
                if self.board[i][j] == 0 {
                    return false;
                }
            }
        }
        true
    }
    /// 返回的值的含义是：0：没有任何作用的操作，例如左键数字、踩雷。  
    /// 1：推进了局面，但没有改变ai对局面的判断，特指标雷。  
    /// 2：改变局面的操作，左键、双击。
    /// 3: 正确的双击.   
    /// e的类型有11种，lc（左键按下）, lr（左键抬起）, rc（右键按下）, rr（右键抬起）, mc（中键按下）, mr（中键抬起）,   
    ///     cc（双键按下，但不确定是哪个键），pf（在开始前预先标雷，而又失去了标记的过程）,   
    ///     l（左键按下或抬起）, r（右键按下或抬起）, m（中键按下或抬起）。  
    /// ## 注意事项：
    /// - 在理想的鼠标状态机中，有些情况是不可能的，例如右键没有抬起就按下两次，但在阿比特中就观察到这种事情。
    // 局面外按下的事件，以及连带的释放一律对鼠标状态没有任何影响，UI框架不会激活回调
    pub fn step(&mut self, e: &str, pos: (usize, usize)) -> Result<u8, ()> {
        // println!("e: {:?}", e);
        if pos.0 == self.row && pos.1 == self.column && (e == "rc" || e == "lc" || e == "cc") {
            // 这里按理应该报错，局面外的按下不该进来
            return Ok(0);
        }
        match self.game_board_state {
            GameBoardState::Ready => match e {
                "mv" => {
                    return Ok(0);
                }
                "lc" => {
                    //  "l"处理不了，很复杂，以后再说
                    match self.mouse_state {
                        MouseState::UpUp => {
                            self.game_board_state = GameBoardState::PreFlaging;
                            self.mouse_state = MouseState::DownUp
                        }
                        MouseState::UpDown => self.mouse_state = MouseState::Chording,
                        MouseState::UpDownNotFlag => self.mouse_state = MouseState::ChordingNotFlag,
                        _ => return Err(()),
                    }
                    return Ok(0);
                }
                "pf" => {
                    assert!(
                        self.game_board[pos.0][pos.1] == 10,
                        "按定义，pf不能在标雷上执行。请报告这个奇怪的录像。"
                    );
                    self.pre_flag_num += 1;
                    self.game_board_state = GameBoardState::PreFlaging;
                    return self.right_click(pos.0, pos.1);
                }
                //  "r"处理不了，很复杂，以后再说
                "rc" => match self.mouse_state {
                    MouseState::UpUp => {
                        self.pre_flag_num = 1;
                        self.game_board_state = GameBoardState::PreFlaging;
                        self.mouse_state = MouseState::UpDown;
                        return self.right_click(pos.0, pos.1);
                    }
                    MouseState::DownUpAfterChording => {
                        self.mouse_state = MouseState::Chording;
                        return Ok(0);
                    }
                    _ => return Err(()),
                },
                "lr" => match self.mouse_state {
                    MouseState::Chording | MouseState::ChordingNotFlag => {
                        self.mouse_state = MouseState::UpDown;
                        return Ok(0);
                    }
                    MouseState::DownUpAfterChording => {
                        self.mouse_state = MouseState::UpUp;
                        return Ok(0);
                    }
                    _ => return Err(()),
                },
                "rr" => match self.mouse_state {
                    MouseState::UpDown => {
                        self.mouse_state = MouseState::UpUp;
                        return Ok(0);
                    }
                    MouseState::Chording => {
                        self.mouse_state = MouseState::DownUpAfterChording;
                        return Ok(0);
                    }
                    _ => return Err(()),
                },
                "cc" => {
                    match self.mouse_state {
                        MouseState::DownUp => self.mouse_state = MouseState::Chording,
                        MouseState::DownUpAfterChording => self.mouse_state = MouseState::Chording,
                        MouseState::UpDown => self.mouse_state = MouseState::Chording,
                        MouseState::UpDownNotFlag => self.mouse_state = MouseState::ChordingNotFlag,
                        _ => return Err(()),
                    }
                    return Ok(0);
                }
                _ => return Err(()),
            },
            GameBoardState::PreFlaging => match e {
                "lc" | "rr" | "mv" => {} // 和playing状态下恰好一致，主要是计算点击次数
                "lr" => match self.mouse_state {
                    MouseState::DownUp => {
                        if pos.0 == self.row && pos.1 == self.column {
                            self.mouse_state = MouseState::UpUp;
                            if self.pre_flag_num == 0 {
                                self.game_board_state = GameBoardState::Ready;
                                self.clear_click_num();
                            }
                            return Ok(0);
                        }
                        if self.game_board[pos.0][pos.1] == 10 {
                            self.game_board_state = GameBoardState::Playing;
                        } else {
                            return Ok(0);
                        }
                    }
                    MouseState::Chording
                    | MouseState::DownUpAfterChording
                    | MouseState::ChordingNotFlag
                    | MouseState::UpUp
                    | MouseState::Undefined => {}
                    _ => return Err(()),
                },
                "pf" => {
                    assert!(
                        self.game_board[pos.0][pos.1] == 10,
                        "按定义，pf不能在标雷上执行。请报告这个奇怪的录像。"
                    );
                    self.pre_flag_num += 1;
                    return self.right_click(pos.0, pos.1);
                }
                "rc" => match self.mouse_state {
                    MouseState::UpUp => {
                        self.mouse_state = MouseState::UpDown;
                        if self.game_board[pos.0][pos.1] == 10 {
                            self.pre_flag_num += 1;
                            self.game_board_state = GameBoardState::PreFlaging;
                            return self.right_click(pos.0, pos.1);
                        } else {
                            self.pre_flag_num -= 1;
                            if self.pre_flag_num == 0 {
                                self.game_board_state = GameBoardState::Ready;
                                self.flag = 0;
                                self.flagedList.clear();
                                self.double = 0;
                                self.left = 0;
                                self.right = 0;
                                self.game_board[pos.0][pos.1] = 10;
                                return Ok(0);
                            } else {
                                return self.right_click(pos.0, pos.1);
                            }
                        }
                    }
                    MouseState::DownUp => {
                        if self.pre_flag_num == 0 {
                            self.game_board_state = GameBoardState::Ready;
                        }
                        self.mouse_state = MouseState::Chording
                    }
                    MouseState::DownUpAfterChording => self.mouse_state = MouseState::Chording,
                    _ => return Err(()),
                },
                "cc" => {
                    match self.mouse_state {
                        MouseState::DownUp => {
                            if self.pre_flag_num == 0 {
                                self.game_board_state = GameBoardState::Ready;
                            }
                            self.mouse_state = MouseState::Chording;
                        }
                        MouseState::DownUpAfterChording => self.mouse_state = MouseState::Chording,
                        MouseState::UpDown => self.mouse_state = MouseState::Chording,
                        MouseState::UpDownNotFlag => self.mouse_state = MouseState::ChordingNotFlag,
                        _ => return Err(()),
                    }
                    return Ok(0);
                }
                _ => return Err(()),
            },
            GameBoardState::Playing => {}
            _ => return Ok(0),
        }
        match e {
            "lc" => match self.mouse_state {
                MouseState::UpUp => self.mouse_state = MouseState::DownUp,
                MouseState::UpDown => self.mouse_state = MouseState::Chording,
                MouseState::UpDownNotFlag => self.mouse_state = MouseState::ChordingNotFlag,
                // 以下情况其实是不可能的
                MouseState::DownUp => {}
                MouseState::DownUpAfterChording => {}
                MouseState::Chording => {}
                MouseState::ChordingNotFlag => {}
                MouseState::Undefined => self.mouse_state = MouseState::DownUp,
            },
            "lr" => match self.mouse_state {
                MouseState::DownUp => {
                    self.mouse_state = MouseState::UpUp;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    // println!("x={:?}, y={:?}", pos.0, pos.1);
                    return self.left_click(pos.0, pos.1);
                }
                MouseState::Chording => {
                    self.mouse_state = MouseState::UpDown;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                MouseState::DownUpAfterChording => self.mouse_state = MouseState::UpUp,
                MouseState::ChordingNotFlag => {
                    self.mouse_state = MouseState::UpDown;
                    self.right -= 1;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                // 以下情况其实是不可能的
                MouseState::UpDown => {}
                MouseState::UpDownNotFlag => {}
                MouseState::UpUp => self.mouse_state = MouseState::UpUp,
                MouseState::Undefined => self.mouse_state = MouseState::UpUp,
            },
            "l" => match self.mouse_state {
                MouseState::DownUp => {
                    self.mouse_state = MouseState::UpUp;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    // println!("x={:?}, y={:?}", pos.0, pos.1);
                    return self.left_click(pos.0, pos.1);
                }
                MouseState::Chording => {
                    self.mouse_state = MouseState::UpDown;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                MouseState::DownUpAfterChording => self.mouse_state = MouseState::UpUp,
                MouseState::ChordingNotFlag => {
                    self.mouse_state = MouseState::UpDown;
                    self.right -= 1;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                MouseState::UpUp => self.mouse_state = MouseState::DownUp,
                MouseState::UpDown => self.mouse_state = MouseState::Chording,
                MouseState::UpDownNotFlag => self.mouse_state = MouseState::ChordingNotFlag,
                MouseState::Undefined => self.mouse_state = MouseState::UpUp,
            },
            "rc" => match self.mouse_state {
                MouseState::UpUp => {
                    if self.game_board[pos.0][pos.1] < 10 {
                        self.mouse_state = MouseState::UpDownNotFlag;
                    } else {
                        self.mouse_state = MouseState::UpDown;
                    }
                    return self.right_click(pos.0, pos.1);
                }
                MouseState::DownUp => self.mouse_state = MouseState::Chording,
                MouseState::DownUpAfterChording => self.mouse_state = MouseState::Chording,
                // 以下情况其实是不可能的
                MouseState::UpDown => {}
                MouseState::UpDownNotFlag => {}
                MouseState::Chording => {}
                MouseState::ChordingNotFlag => {}
                MouseState::Undefined => self.mouse_state = MouseState::UpDown,
            },
            "rr" => match self.mouse_state {
                MouseState::UpDown => self.mouse_state = MouseState::UpUp,
                MouseState::UpDownNotFlag => self.mouse_state = MouseState::UpUp,
                MouseState::Chording => {
                    self.mouse_state = MouseState::DownUpAfterChording;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                MouseState::ChordingNotFlag => {
                    self.mouse_state = MouseState::DownUpAfterChording;
                    self.right -= 1;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                // 以下情况其实是不可能的
                MouseState::DownUp => {}
                MouseState::DownUpAfterChording => {}
                MouseState::UpUp => self.mouse_state = MouseState::UpUp,
                MouseState::Undefined => self.mouse_state = MouseState::UpUp,
            },
            "r" => match self.mouse_state {
                MouseState::UpDown => self.mouse_state = MouseState::UpUp,
                MouseState::UpDownNotFlag => self.mouse_state = MouseState::UpUp,
                MouseState::Chording => {
                    self.mouse_state = MouseState::DownUpAfterChording;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                MouseState::ChordingNotFlag => {
                    self.mouse_state = MouseState::DownUpAfterChording;
                    self.right -= 1;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                // 以下情况其实是不可能的
                MouseState::UpUp => {
                    if self.game_board[pos.0][pos.1] < 10 {
                        self.mouse_state = MouseState::UpDownNotFlag;
                    } else {
                        self.mouse_state = MouseState::UpDown;
                    }
                    return self.right_click(pos.0, pos.1);
                }
                MouseState::DownUp => self.mouse_state = MouseState::Chording,
                MouseState::DownUpAfterChording => self.mouse_state = MouseState::Chording,
                MouseState::Undefined => self.mouse_state = MouseState::UpUp,
            },
            "mv" => {}
            "mc" => {
                self.middle_hold = true;
            }
            "mr" => {
                self.middle_hold = false;
                return self.chording_click(pos.0, pos.1);
            }
            "m" => {
                self.middle_hold = !self.middle_hold;
                if !self.middle_hold {
                    return self.chording_click(pos.0, pos.1);
                }
            }
            "cc" => match self.mouse_state {
                MouseState::DownUp => self.mouse_state = MouseState::Chording,
                MouseState::DownUpAfterChording => self.mouse_state = MouseState::Chording,
                MouseState::UpDown => self.mouse_state = MouseState::Chording,
                MouseState::UpDownNotFlag => self.mouse_state = MouseState::ChordingNotFlag,
                _ => return Err(()),
            },
            "crl" => match self.mouse_state {
                MouseState::Chording => {
                    self.mouse_state = MouseState::UpDown;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                MouseState::ChordingNotFlag => {
                    self.mouse_state = MouseState::UpDown;
                    self.right -= 1;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                _ => return Err(()),
            },
            "crr" => match self.mouse_state {
                MouseState::Chording => {
                    self.mouse_state = MouseState::DownUpAfterChording;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                MouseState::ChordingNotFlag => {
                    self.mouse_state = MouseState::DownUpAfterChording;
                    self.right -= 1;
                    if pos.0 == self.row && pos.1 == self.column {
                        return Ok(0);
                    }
                    return self.chording_click(pos.0, pos.1);
                }
                _ => return Err(()),
            },
            _ => return Err(()),
        }
        Ok(0)
    }
    /// 直接分析整局的操作流，中间也可以停顿
    /// 开始游戏前的任何操作也都记录次数
    pub fn step_flow(&mut self, operation: Vec<(&str, (usize, usize))>) -> Result<(), ()> {
        for op in operation {
            self.step(op.0, op.1)?;
        }
        Ok(())
    }
    fn is_win(&mut self) -> bool {
        for j in self.pointer_y..self.column {
            if self.game_board[self.pointer_x][j] >= 10 && self.board[self.pointer_x][j] != -1 {
                self.pointer_y = j;
                return false;
            }
        }
        for i in self.pointer_x + 1..self.row {
            for j in 0..self.column {
                if self.game_board[i][j] >= 10 && self.board[i][j] != -1 {
                    self.pointer_x = i;
                    self.pointer_y = j;
                    return false;
                }
            }
        }
        true
    }
    /// 初始化。对应强化学习领域gym的api中的reset。
    pub fn reset(&mut self) {
        self.game_board = vec![vec![10; self.column]; self.row];
        self.board = vec![vec![0; self.column]; self.row];
        self.left = 0;
        self.right = 0;
        self.double = 0;
        self.ce = 0;
        self.flag = 0;
        self.left = 0;
        self.bbbv_solved = 0;
        self.flagedList = vec![];
        self.mouse_state = MouseState::UpUp;
        self.game_board_state = GameBoardState::Ready;
        self.pointer_x = 0;
        self.pointer_y = 0;
    }
    // 清空状态机里的点击次数
    fn clear_click_num(&mut self) {
        self.flag = 0;
        self.flagedList.clear();
        self.double = 0;
        self.left = 0;
        self.right = 0;
    }
}

/// 鼠标状态
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum MouseState {
    UpUp,
    UpDown,
    /// 右键按下，且既没有标雷，也没有取消标雷的状态
    UpDownNotFlag,
    DownUp,
    /// 双键都按下的其他状态
    Chording,
    /// 双键都按下，且是在不可以右击的格子上、先按下右键
    ChordingNotFlag,
    /// 双击后先弹起右键，左键还没弹起的状态
    DownUpAfterChording,
    /// 未初始化的状态
    Undefined,
}

/// 游戏局面状态
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GameBoardState {
    Ready,
    /// 游戏开始，埋雷前标雷，将被记录到录像里。
    PreFlaging,
    Playing,
    Loss,
    Win,
    /// 局面作为录像在被播放
    Display,
}

/// 读录像文件失败的原因
#[derive(Debug)]
pub enum ErrReadVideoReason {
    CanNotFindFile,
    FileIsTooShort,
    FileIsNotRmv,
    FileIsEmpty,
    InvalidBoardSize,
    InvalidLevel,
    InvalidParams,
    InvalidVideoEvent,
    InvalidMinePosition,
}

/// 局面活动（点击或移动）
// pub struct Event {
//     pub time: f64,
//     /// 操作类型，这几种："mv", "lc", "lr", "rc", "rr", "mc", "mr", "pf"
//     pub mouse: String,
//     /// 距离左端有几像素。
//     pub x: u16,
//     /// 距离上端有几像素。
//     pub y: u16,
// }

/// 录像里的局面活动（点击或移动）、指标状态(该活动完成后的)、先验后验局面索引
pub struct VideoActionStateRecorder {
    pub time: f64,
    /// 操作类型，这几种："mv", "lc", "lr", "rc", "rr", "mc", "mr", "pf"
    pub mouse: String,
    /// 距离左端有几像素。
    pub x: u16,
    /// 距离上端有几像素。
    pub y: u16,
    /// 0代表完全没用，
    /// 1代表能仅推进局面但不改变对局面的后验判断，例如标雷和取消标雷
    /// 2代表改变对局面的后验判断的操作，例如左键点开一个或一片格子，不包括双击
    /// 3代表有效、至少打开了一个格子的双击
    /// 和ce没有关系，仅用于控制计算
    pub useful_level: u8,
    /// 操作前的局面（先验局面）的索引。
    pub prior_game_board_id: usize,
    /// 操作后的局面（后验的局面）的索引。
    pub next_game_board_id: usize,
    pub comments: String,
    /// 该操作完成以后的鼠标状态。和录像高亮有关。即使是鼠标move也会记录。
    pub mouse_state: MouseState,
    /// 该操作完成以后，已解决的3BV。
    // pub solved3BV: usize,
    /// 指标状态(该活动完成后的、后验的), mv也会记录，浪费了空间
    pub key_dynamic_params: KeyDynamicParams,
    pub path: f64,
}

impl Default for VideoActionStateRecorder {
    fn default() -> Self {
        VideoActionStateRecorder {
            time: 0.0,
            mouse: "".to_string(),
            x: 0,
            y: 0,
            useful_level: 0,
            prior_game_board_id: 0,
            next_game_board_id: 0,
            comments: "".to_string(),
            mouse_state: MouseState::Undefined,
            key_dynamic_params: KeyDynamicParams::default(),
            path: 0.0,
        }
    }
}

pub struct StaticParams {
    pub bbbv: usize,
    pub op: usize,
    pub isl: usize,
    pub hizi: usize,
    pub cell0: usize,
    pub cell1: usize,
    pub cell2: usize,
    pub cell3: usize,
    pub cell4: usize,
    pub cell5: usize,
    pub cell6: usize,
    pub cell7: usize,
    pub cell8: usize,
    /// 鼠标回报率
    pub fps: usize,
}

impl Default for StaticParams {
    fn default() -> Self {
        StaticParams {
            bbbv: 0,
            op: 0,
            isl: 0,
            hizi: 0,
            cell0: 0,
            cell1: 0,
            cell2: 0,
            cell3: 0,
            cell4: 0,
            cell5: 0,
            cell6: 0,
            cell7: 0,
            cell8: 0,
            fps: 0,
        }
    }
}

/// 侧重实时记录中间过程、中间状态
pub struct KeyDynamicParams {
    pub left: usize,
    pub right: usize,
    pub double: usize,
    pub ce: usize,
    pub flag: usize,
    pub bbbv_solved: usize,
    pub op_solved: usize,
    pub isl_solved: usize,
}

impl Default for KeyDynamicParams {
    fn default() -> Self {
        KeyDynamicParams {
            left: 0,
            right: 0,
            double: 0,
            ce: 0,
            flag: 0,
            bbbv_solved: 0,
            op_solved: 0,
            isl_solved: 0,
        }
    }
}

/// 游戏动态类指标，侧重保存最终结果
pub struct GameDynamicParams {
    /// 最终时间成绩，不是时间的函数
    pub rtime: f64,
    /// 以毫秒为单位的精确时间
    pub rtime_ms: u32,
    pub left: usize,
    pub right: usize,
    pub double: usize,
    pub cl: usize,
    pub flag: usize,
    pub left_s: f64,
    pub right_s: f64,
    pub double_s: f64,
    pub cl_s: f64,
    pub flag_s: f64,
    /// 四舍五入折算到16像素边长，最终路径长度
    pub path: usize,
}

impl Default for GameDynamicParams {
    fn default() -> Self {
        GameDynamicParams {
            rtime: 0.0,
            rtime_ms: 0,
            left: 0,
            right: 0,
            double: 0,
            cl: 0,
            flag: 0,
            left_s: 0.0,
            right_s: 0.0,
            double_s: 0.0,
            cl_s: 0.0,
            flag_s: 0.0,
            path: 0,
        }
    }
}

/// 录像动态类指标，侧重保存最终结果
pub struct VideoDynamicParams {
    pub etime: f64,
    pub bbbv_s: f64,
    pub bbbv_solved: usize,
    pub stnb: f64,
    pub rqp: f64,
    pub qg: f64,
    pub ce: usize,
    pub ce_s: f64,
    pub ioe: f64,
    pub corr: f64,
    pub thrp: f64,
    pub op_solved: usize,
    pub isl_solved: usize,
}

impl Default for VideoDynamicParams {
    fn default() -> Self {
        VideoDynamicParams {
            etime: 0.0,
            bbbv_s: 0.0,
            bbbv_solved: 0,
            stnb: 0.0,
            rqp: 0.0,
            qg: 0.0,
            ce: 0,
            ce_s: 0.0,
            ioe: 0.0,
            corr: 0.0,
            thrp: 0.0,
            op_solved: 0,
            isl_solved: 0,
        }
    }
}

pub struct BaseVideo {
    /// 软件名
    pub software: Vec<u8>,
    /// 宽度，等同于column
    pub width: usize,
    /// 高度，等同于row
    pub height: usize,
    /// 雷数
    pub mine_num: usize,
    /// 是否扫完: 软件证明这局是完成的，即没有踩雷、时间等所有数值没有溢出。其余条件一概不保证。  
    /// 初始是false，踩雷的话，分析完还是false
    pub is_completed: bool,
    /// 是否正式: 软件证明这局是正式的，一定扫完，包括没有用软件筛选3BV、没有看概率、是标准模式、时间等所有数值没有溢出。  
    /// 不一定包括是否满足排名网站对于3BV的额外限制。例如，没有人能证明avf录像是否正式。
    pub is_offical: bool,
    /// 是否公平完成。软件证明这局是公平完成的，一定扫完，比如没有用软件筛选3BV、没有看概率、时间等所有数值没有溢出。  
    /// 公平完成和正式的区别是，只有标准游戏模式可以是正式的，而upk、无猜等模式不正式，但可以是公平完成的。  
    /// 如果是正式的，则一定是公平完成的。
    pub is_fair: bool,
    /// 是不是盲扫，初始是false，不是盲扫的话，分析完还是false
    pub nf: bool,
    /// 游戏模式。0->标准、1->upk；2->cheat；3->Density（来自Viennasweeper、clone软件）、4->win7、5->竞速无猜、6->强无猜、7->弱无猜、8->准无猜、9->强可猜、10->弱可猜
    pub mode: u16,
    /// 游戏难度（级别）。3是初级；4是中级；5是高级；6是自定义。
    pub level: u8,
    /// 逻辑上，方格像素的尺寸。传统扫雷一般是16，只有元扫雷会把逻辑尺寸和实际尺寸统一起来。
    pub cell_pixel_size: u8,
    /// 仅在存录像时用到。貌似可以不用。
    pub board: Vec<Vec<i32>>,
    /// 局面状态机
    pub minesweeper_board: MinesweeperBoard,
    /// 录像状态。貌似用不到。
    pub game_board_state: GameBoardState,
    /// 动作、状态记录器
    pub video_action_state_recorder: Vec<VideoActionStateRecorder>,
    /// 游戏局面流，从一开始没有打开任何格子（包含玩家游戏前的标雷过程），到最后打开了所有
    pub game_board_stream: Vec<GameBoard>,
    /// 游戏开始的时间，由计时器控制，仅游戏时用
    pub video_start_instant: Instant,
    /// 第一次有效的左键抬起的时间，由计时器控制，仅游戏时用, new_before_game方法里用到，真正开始的时间
    pub game_start_instant: Instant,
    /// 第一次有效的左键抬起的时间，格式不同，只在录像播放模式用到
    delta_time: f64,
    /// 当前时间，仅录像播放时用
    pub current_time: f64,
    /// 当前指针指向
    pub current_event_id: usize,
    /// 录像用户标识
    pub player_designator: Vec<u8>,
    /// 比赛标识
    pub race_designator: Vec<u8>,
    /// 唯一性标识
    pub uniqueness_designator: Vec<u8>,
    /// 游戏起始时间和终止时间。不整理格式，读成字符串。
    /// 举例：在阿比特中，‘16.10.2021.22.24.23.9906’，意味2021年10月16日，下午10点24分23秒9906。
    /// 维也纳扫雷中，‘1382834716’，代表以秒为单位的时间戳
    pub start_time: Vec<u8>,
    /// 维也纳扫雷中没有
    pub end_time: Vec<u8>,
    /// 国家。预留字段，暂时不能解析。
    pub country: Vec<u8>,
    /// 原始二进制数据
    raw_data: Vec<u8>,
    /// 解析二进制文件数据时的指针
    pub offset: usize,
    /// 静态指标
    pub static_params: StaticParams,
    /// 最终的游戏动态指标
    game_dynamic_params: GameDynamicParams,
    /// 最终的录像动态指标
    video_dynamic_params: VideoDynamicParams,
    // /// 最终的路径长度
    // pub path: usize,
    // /// 开始扫前，已经标上的雷。如果操作流中包含标这些雷的过程，
    // pub pre_flags: Vec<(usize, usize)>,
    ///校验码
    pub checksum: [u8; 32],
    pub can_analyse: bool,
    // 游戏前标的雷数
    // new_before_game方法里用到，真正开始的时间
    // net_start_time: f64,
    // 允许设置最终成绩，解析录像文件时用
    allow_set_rtime: bool,
    // 播放录像文件时用，按几倍放大来播放，涉及回报的鼠标位置
    video_playing_pix_size_k: f64,
}

impl Default for BaseVideo {
    fn default() -> Self {
        BaseVideo {
            software: vec![],
            width: 0,
            height: 0,
            mine_num: 0,
            is_completed: false,
            is_offical: false,
            is_fair: false,
            nf: false,
            mode: 0,
            level: 0,
            cell_pixel_size: 16,
            board: vec![],
            minesweeper_board: MinesweeperBoard::default(),
            game_board_state: GameBoardState::Display,
            video_action_state_recorder: vec![],
            game_board_stream: vec![],
            video_start_instant: Instant::now(),
            game_start_instant: Instant::now(),
            delta_time: 0.0,
            current_time: 0.0,
            current_event_id: 0,
            player_designator: vec![],
            race_designator: vec![],
            uniqueness_designator: vec![],
            start_time: vec![],
            end_time: vec![],
            country: vec![],
            raw_data: vec![],
            offset: 0,
            static_params: StaticParams::default(),
            game_dynamic_params: GameDynamicParams::default(),
            video_dynamic_params: VideoDynamicParams::default(),
            checksum: [0; 32],
            can_analyse: false,
            // net_start_time: 0.0,
            allow_set_rtime: false,
            video_playing_pix_size_k: 1.0,
        }
    }
}

impl BaseVideo {
    pub fn get_u8(&mut self) -> Result<u8, ErrReadVideoReason> {
        let t = self.raw_data.get(self.offset);
        self.offset += 1;
        match t {
            Some(x) => Ok(*x),
            None => Err(ErrReadVideoReason::FileIsTooShort),
        }
    }
    /// 都是大端法
    pub fn get_u16(&mut self) -> Result<u16, ErrReadVideoReason> {
        let a = self.get_u8()?;
        let b = self.get_u8()?;
        Ok((a as u16) << 8 | (b as u16))
    }
    pub fn get_u24(&mut self) -> Result<u32, ErrReadVideoReason> {
        let a = self.get_u8()?;
        let b = self.get_u8()?;
        let c = self.get_u8()?;
        Ok((a as u32) << 16 | (b as u32) << 8 | (c as u32))
    }
    pub fn get_u32(&mut self) -> Result<u32, ErrReadVideoReason> {
        let a = self.get_u8()?;
        let b = self.get_u8()?;
        let c = self.get_u8()?;
        let d = self.get_u8()?;
        Ok((a as u32) << 24 | (b as u32) << 16 | (c as u32) << 8 | (d as u32))
    }
    pub fn get_char(&mut self) -> Result<char, ErrReadVideoReason> {
        let a = self.get_u8()?;
        Ok(a as char)
    }
}

impl BaseVideo {
    /// 通过文件名构造。
    #[cfg(any(feature = "py", feature = "rs"))]
    pub fn new_with_file(file_name: &str) -> BaseVideo {
        let raw_data: Vec<u8> = fs::read(file_name).unwrap();
        // for i in 0..500 {
        //     print!("{:?}", raw_data[i] as char);
        // }
        BaseVideo {
            raw_data,
            allow_set_rtime: true,
            ..BaseVideo::default()
        }
    }
    /// 游戏前实例化，游戏中不断调用step方法来维护。
    #[cfg(any(feature = "py", feature = "rs"))]
    pub fn new_before_game(board: Vec<Vec<i32>>, cell_pixel_size: u8) -> BaseVideo {
        let bbbv = cal_bbbv(&board);
        // 这里算出来的雷数不一定准
        let mine_num = board.iter().fold(0, |y, row| {
            y + row
                .iter()
                .fold(0, |yy, x| if *x == -1 { yy + 1 } else { yy })
        });
        let board_clone = board.clone();
        BaseVideo {
            width: board[0].len(),
            height: board.len(),
            mine_num,
            cell_pixel_size,
            board,
            minesweeper_board: MinesweeperBoard::new(board_clone),
            game_board_state: GameBoardState::Ready,
            static_params: StaticParams {
                bbbv,
                ..StaticParams::default()
            },
            ..BaseVideo::default()
        }
    }
    #[cfg(feature = "js")]
    pub fn new(video_data: Vec<u8>) -> BaseVideo {
        // video_data = video_data.into_vec();
        BaseVideo {
            video_data,
            ..BaseVideo::default()
        }
    }
    /// 步进
    /// - pos的单位是像素，(距离上方，距离左侧)
    pub fn step(&mut self, e: &str, pos: (usize, usize)) -> Result<u8, ()> {
        let step_instant = Instant::now();
        // 这是和录像时间戳有关
        let mut time_ms = step_instant
            .duration_since(self.video_start_instant)
            .as_millis() as u32;
        let mut time = time_ms as f64 / 1000.0;
        let old_state = self.minesweeper_board.game_board_state;
        if old_state == GameBoardState::Loss
            || old_state == GameBoardState::Win
            || old_state == GameBoardState::Display
        {
            return Ok(0);
        }
        // self.minesweeper_board.step(e, (pos.0 / self.cell_pixel_size as usize, pos.1 / self.cell_pixel_size as usize));
        let x = pos.0 / self.cell_pixel_size as usize;
        let y = pos.1 / self.cell_pixel_size as usize;

        let a = self.minesweeper_board.step(e, (x, y))?;
        self.game_board_state = self.minesweeper_board.game_board_state;
        match self.game_board_state {
            GameBoardState::Ready => {
                self.game_board_stream.clear();
                self.video_action_state_recorder.clear();
                return Ok(0);
            }
            GameBoardState::PreFlaging => {
                if old_state != GameBoardState::PreFlaging {
                    self.video_start_instant = step_instant;
                    time_ms = step_instant
                        .duration_since(self.video_start_instant)
                        .as_millis() as u32;
                    time = time_ms as f64 / 1000.0;
                }
            }
            GameBoardState::Playing => {
                if old_state != GameBoardState::Playing {
                    if old_state == GameBoardState::Ready {
                        self.video_start_instant = step_instant;
                        time_ms = step_instant
                            .duration_since(self.video_start_instant)
                            .as_millis() as u32;
                        time = time_ms as f64 / 1000.0;
                    }
                    self.game_start_instant = step_instant;
                    // 高精度的时间戳，单位为微秒
                    // https://doc.rust-lang.org/std/time/struct.Instant.html
                    self.start_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_micros()
                        .to_string()
                        .into_bytes();
                }
            }
            // 不可能
            GameBoardState::Display => {}
            GameBoardState::Loss => {
                let t_ms = time_ms_between(step_instant, self.game_start_instant);
                self.end_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
                    .to_string()
                    .into_bytes();
                self.is_completed = false;
                // 这是和录像时间成绩有关
                let t = t_ms as f64 / 1000.0;
                self.static_params.bbbv = cal_bbbv(&self.board);
                self.game_dynamic_params.rtime = t;
                self.game_dynamic_params.rtime_ms = t_ms;
                self.video_dynamic_params.etime =
                    t / self.minesweeper_board.bbbv_solved as f64 * self.static_params.bbbv as f64;

                self.gather_params_after_game(t);
            }
            GameBoardState::Win => {
                let t_ms = time_ms_between(step_instant, self.game_start_instant);
                self.end_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
                    .to_string()
                    .into_bytes();
                self.is_completed = true;
                let t = t_ms as f64 / 1000.0;
                self.static_params.bbbv = cal_bbbv(&self.board);
                self.game_dynamic_params.rtime = t;
                self.game_dynamic_params.rtime_ms = t_ms;
                self.video_dynamic_params.etime = t;

                self.gather_params_after_game(t);
            }
        }
        // 维护path，挺复杂的
        let mut path = 0.0;
        if old_state == GameBoardState::Playing {
            let mut outside_skip = self.video_action_state_recorder.len() - 1;
            loop {
                let p = &self.video_action_state_recorder[outside_skip];
                if !(p.y as usize >= self.height * self.cell_pixel_size as usize
                    || p.x as usize >= self.width * self.cell_pixel_size as usize)
                {
                    path = p.path;
                    path += ((pos.0 as f64 - p.y as f64).powf(2.0)
                        + (pos.1 as f64 - p.x as f64).powf(2.0))
                    .powf(0.5)
                        * 16.0
                        / (self.cell_pixel_size as f64);
                    // println!("path, {:?}", path);
                    // self.current_event_id += 1;
                    // println!("pos.0:{:?}; p.y:{:?}; {:?}; {:?}", pos.0,p.y,pos.1,p.x);
                    break;
                } else {
                    outside_skip -= 1;
                }
            }
            // 开始维护第一个先验局面
        } else if self.game_board_stream.is_empty()
            && (self.minesweeper_board.game_board_state == GameBoardState::PreFlaging
                || self.minesweeper_board.game_board_state == GameBoardState::Playing
                || self.minesweeper_board.game_board_state == GameBoardState::Win
                || self.minesweeper_board.game_board_state == GameBoardState::Loss)
        {
            let mut g_b = GameBoard::new(self.mine_num);
            g_b.set_game_board(&vec![vec![10; self.width]; self.height]);
            self.game_board_stream.push(g_b);
            path = 0.0;
        }
        // self.current_time = time;
        let prior_game_board_id;
        let next_game_board_id;
        if a >= 1 {
            let mut g_b = GameBoard::new(self.mine_num);
            g_b.set_game_board(&self.minesweeper_board.game_board);
            self.game_board_stream.push(g_b);
            next_game_board_id = self.game_board_stream.len() - 1;
            prior_game_board_id = self.game_board_stream.len() - 2;
        } else {
            next_game_board_id = self.game_board_stream.len() - 1;
            prior_game_board_id = self.game_board_stream.len() - 1;
        }
        self.video_action_state_recorder
            .push(VideoActionStateRecorder {
                time,
                mouse: e.to_string(),
                x: pos.1 as u16,
                y: pos.0 as u16,
                next_game_board_id,
                prior_game_board_id,
                useful_level: a,
                comments: "".to_string(),
                mouse_state: self.minesweeper_board.mouse_state,
                key_dynamic_params: KeyDynamicParams {
                    left: self.minesweeper_board.left,
                    right: self.minesweeper_board.right,
                    double: self.minesweeper_board.double,
                    ce: self.minesweeper_board.ce,
                    flag: self.minesweeper_board.flag,
                    bbbv_solved: self.minesweeper_board.bbbv_solved,
                    op_solved: 0,
                    isl_solved: 0,
                },
                path,
            });
        // println!("push: {:?}, {:?}, ({:?}, {:?})", time, e, pos.0, pos.1);
        Ok(0)
    }
    /// 重置游戏状态等，不重置标识等很多局都不会变的数据。点脸等，重开。
    pub fn reset(&mut self, row: usize, column: usize, pix_size: u8) {
        self.game_board_stream.clear();
        self.minesweeper_board = MinesweeperBoard::new(vec![vec![0; column]; row]);
        self.width = column;
        self.height = row;
        self.cell_pixel_size = pix_size;
        self.video_action_state_recorder.clear();
        self.game_board_stream.clear();
        self.raw_data.clear();
        self.static_params = StaticParams::default();
        self.game_dynamic_params = GameDynamicParams::default();
        self.video_dynamic_params = VideoDynamicParams::default();
        self.game_board_state = GameBoardState::Ready;
    }
    /// 获胜后标上所有的雷，没获胜则返回
    pub fn win_then_flag_all_mine(&mut self) {
        if self.minesweeper_board.game_board_state != GameBoardState::Win {
            return;
        }
        self.minesweeper_board.game_board.iter_mut().for_each(|x| {
            x.iter_mut().for_each(|xx| {
                if *xx == 10 {
                    *xx = 11
                }
            })
        });
    }
    /// 失败后展示所有的雷，没失败则返回
    /// 这件事状态机不会自动做，因为有些模式失败后不标出所有的雷，如强无猜
    pub fn loss_then_open_all_mine(&mut self) {
        if self.minesweeper_board.game_board_state != GameBoardState::Loss {
            return;
        }
        for i in 0..self.height {
            for j in 0..self.width {
                if self.minesweeper_board.board[i][j] == -1
                    && self.minesweeper_board.game_board[i][j] == 10
                {
                    self.minesweeper_board.game_board[i][j] = 16;
                }
            }
        }
    }
    /// 游戏结束后，计算一批指标
    fn gather_params_after_game(&mut self, t: f64) {
        self.game_dynamic_params.left = self.minesweeper_board.left;
        self.game_dynamic_params.right = self.minesweeper_board.right;
        self.game_dynamic_params.double = self.minesweeper_board.double;
        self.game_dynamic_params.flag = self.minesweeper_board.flag;
        self.game_dynamic_params.cl = self.minesweeper_board.left
            + self.game_dynamic_params.right
            + self.game_dynamic_params.double;
        self.game_dynamic_params.left_s = self.minesweeper_board.left as f64 / t;
        self.game_dynamic_params.right_s = self.minesweeper_board.right as f64 / t;
        self.game_dynamic_params.double_s = self.minesweeper_board.double as f64 / t;
        self.game_dynamic_params.flag_s = self.minesweeper_board.flag as f64 / t;
        self.game_dynamic_params.cl_s = self.game_dynamic_params.cl as f64 / t;
        let cell_nums = cal_cell_nums(&self.board);
        self.static_params.cell0 = cell_nums[0];
        self.static_params.cell1 = cell_nums[1];
        self.static_params.cell2 = cell_nums[2];
        self.static_params.cell3 = cell_nums[3];
        self.static_params.cell4 = cell_nums[4];
        self.static_params.cell5 = cell_nums[5];
        self.static_params.cell6 = cell_nums[6];
        self.static_params.cell7 = cell_nums[7];
        self.static_params.cell8 = cell_nums[8];
        self.static_params.op = cal_op(self.board.clone());
        self.static_params.isl = cal_isl(&self.board);
    }
    /// 进行局面的推衍，计算基本的局面参数，记录所有中间过程。不包含概率计算。
    /// - 对于avf录像，必须analyse以后才能正确获取是否扫完。
    pub fn analyse(&mut self) {
        // println!("{:?}, ", self.board);
        assert!(
            self.can_analyse,
            "调用parse_video或扫完前，不能调用analyse方法"
        );
        // self.minesweeper_board
        let mut b = MinesweeperBoard::new(self.board.clone());
        let mut first_game_board = GameBoard::new(self.mine_num);
        first_game_board.set_game_board(&vec![vec![10; self.width]; self.height]);
        self.game_board_stream.push(first_game_board);
        for ide in 0..self.video_action_state_recorder.len() {
            // 控制svi的生命周期
            let mut svi = &mut self.video_action_state_recorder[ide];
            svi.prior_game_board_id = self.game_board_stream.len() - 1;
            if svi.mouse != "mv" {
                let old_state = b.game_board_state;
                // println!("{:?}, {:?}", svi.mouse, svi.y);
                let u_level = b
                    .step(
                        &svi.mouse,
                        (
                            (svi.y / self.cell_pixel_size as u16) as usize,
                            (svi.x / self.cell_pixel_size as u16) as usize,
                        ),
                    )
                    .unwrap();
                // println!("{:?}, {:?}", svi.mouse, b.game_board);
                svi.useful_level = u_level;
                if u_level >= 1 {
                    let mut g_b = GameBoard::new(self.mine_num);
                    g_b.set_game_board(&b.game_board);
                    self.game_board_stream.push(g_b);
                    if old_state != GameBoardState::Playing {
                        self.delta_time = svi.time;
                    }
                    // println!("{:?}, {:?}", self.game_board_stream.len(), svi.mouse);
                }
            }
            svi.next_game_board_id = self.game_board_stream.len() - 1;
            svi.mouse_state = b.mouse_state.clone();
            svi.key_dynamic_params.left = b.left;
            svi.key_dynamic_params.right = b.right;
            svi.key_dynamic_params.bbbv_solved = b.bbbv_solved;
            svi.key_dynamic_params.double = b.double;
            svi.key_dynamic_params.ce = b.ce;
            svi.key_dynamic_params.flag = b.flag;
            // 这两个很难搞
            svi.key_dynamic_params.op_solved = 0;
            svi.key_dynamic_params.isl_solved = 0;
            let svi = &self.video_action_state_recorder[ide];
            // 第一下操作不可能是在局面外的
            if b.game_board_state == GameBoardState::Playing
                || b.game_board_state == GameBoardState::Win
                || b.game_board_state == GameBoardState::Loss
            {
                if svi.y >= self.height as u16 * self.cell_pixel_size as u16
                    && svi.x >= self.width as u16 * self.cell_pixel_size as u16
                {
                    self.video_action_state_recorder[ide].path =
                        self.video_action_state_recorder[ide - 1].path;
                } else {
                    let mut svi_1_y: u16;
                    let mut svi_1_x: u16;
                    let mut svi_1_path: f64;
                    let mut skip = 1;
                    loop {
                        let svis = &self.video_action_state_recorder[ide - skip];
                        svi_1_y = svis.y; // 距离上端
                        svi_1_x = svis.x;
                        svi_1_path = svis.path;
                        if svi_1_y >= self.height as u16 * self.cell_pixel_size as u16
                            && svi_1_x >= self.width as u16 * self.cell_pixel_size as u16
                        {
                            skip += 1;
                            continue;
                        } else {
                            break;
                        };
                    }
                    let mut svi = &mut self.video_action_state_recorder[ide];
                    svi.path = svi_1_path
                        + ((svi.y as f64 - svi_1_y as f64).powf(2.0)
                            + (svi.x as f64 - svi_1_x as f64).powf(2.0))
                        .powf(0.5)
                            * 16.0
                            / (self.cell_pixel_size as f64);
                }
            }
        }
        self.is_completed = b.game_board_state == GameBoardState::Win;
        self.game_dynamic_params.left = b.left;
        self.game_dynamic_params.left_s = b.left as f64 / self.game_dynamic_params.rtime;
        self.game_dynamic_params.right = b.right;
        self.game_dynamic_params.right_s = b.right as f64 / self.game_dynamic_params.rtime;
        // println!("---{:?}", b.bbbv_solved);
        self.video_dynamic_params.bbbv_solved = b.bbbv_solved;
        self.video_dynamic_params.ce = b.ce;
        self.video_dynamic_params.ce_s = b.ce as f64 / self.game_dynamic_params.rtime;
        self.game_dynamic_params.double = b.double;
        self.game_dynamic_params.cl = b.left + b.right + b.double;
        self.game_dynamic_params.cl_s =
            self.game_dynamic_params.cl as f64 / self.game_dynamic_params.rtime;
        self.game_dynamic_params.flag = b.flag;
        self.video_dynamic_params.bbbv_s =
            self.static_params.bbbv as f64 / self.game_dynamic_params.rtime;
        self.video_dynamic_params.rqp = self.game_dynamic_params.rtime
            * self.game_dynamic_params.rtime
            / self.static_params.bbbv as f64;
        if self.height == 8 && self.width == 8 && self.mine_num == 10 {
            self.video_dynamic_params.stnb = 47.22
                / (self.game_dynamic_params.rtime.powf(1.7) / self.static_params.bbbv as f64)
                * (b.bbbv_solved as f64 / self.static_params.bbbv as f64).powf(0.5);
        } else if self.height == 16 && self.width == 16 && self.mine_num == 40 {
            self.video_dynamic_params.stnb = 153.73
                / (self.game_dynamic_params.rtime.powf(1.7) / self.static_params.bbbv as f64)
                * (b.bbbv_solved as f64 / self.static_params.bbbv as f64).powf(0.5);
        } else if self.height == 16 && self.width == 30 && self.mine_num == 99 {
            self.video_dynamic_params.stnb = 435.001
                / (self.game_dynamic_params.rtime.powf(1.7) / self.static_params.bbbv as f64)
                * (b.bbbv_solved as f64 / self.static_params.bbbv as f64).powf(0.5);
        } // 凡自定义的stnb都等于0
        self.video_dynamic_params.ioe = b.bbbv_solved as f64 / self.game_dynamic_params.cl as f64;
        self.video_dynamic_params.corr = b.ce as f64 / self.game_dynamic_params.cl as f64;
        self.video_dynamic_params.thrp = b.bbbv_solved as f64 / b.ce as f64;
    }
    /// 传入要检查的事件，会把结果记在comments字段里。
    /// 可以传入high_risk_guess、jump_judge、needless_guess、mouse_trace、vision_transfer、survive_poss等。顺序不讲究。
    /// #### 检查录像中所有的教科书式的fl局部（python）
    /// ```python
    /// import ms_toollib as ms
    /// import re
    /// v = ms.AvfVideo("z.avf"); # 用文件名实例化
    /// v.parse_video()
    /// v.analyse()
    /// v.analyse_for_features(["super_fl_local"]) # 用哪些分析方法。分析结果会记录到events_comments字段里
    /// for i in range(v.events_len): # 鼠标事件包括鼠标移动、按下、抬起
    ///     c = v.events_comments(i) # 每一个鼠标事件，都有events_comments字段，记录了分析算法的结果
    ///     if c != '':
    ///         print('时间：', v.events_time(i), '事件：', c)
    ///         step_num = int(re.findall(r"(?<=步数).*?(?=\))", c)[0]) # 正则提取要打印几步
    ///         p = i
    ///         for j in range(step_num):
    ///             while v.events_useful_level(p) <= 0:
    ///                 # 用events_useful_level字段辅助过滤不要看的事件
    ///                 # 也可以用events_mouse_state字段来过滤
    ///                 p += 1
    ///             print('鼠标事件类型：', v.events_mouse(p),
    ///                 '第几行：', v.events_y(p)//16,
    ///                 '第几列：', v.events_x(p)//16)
    ///             p += 1
    /// ```
    pub fn analyse_for_features(&mut self, controller: Vec<&str>) {
        // 事件分析，返回一个向量，格式是event索引、字符串event的类型
        // error: 高风险的猜雷（猜对概率0.05）
        // feature: 跳判
        // error: 判雷失败
        // feature: 双线操作
        // feature: 破空（成功率0.98）
        // error: 鼠标大幅绕路(500%)
        // warning：鼠标绕路(200%)
        // warning: 可以判雷时选择猜雷
        // suspect: 点击速度过快(0.01)
        // suspect: 鼠标移动过快(2)
        //
        for o in controller {
            match o {
                "high_risk_guess" => analyse_high_risk_guess(self),
                "jump_judge" => analyse_jump_judge(self),
                "needless_guess" => analyse_needless_guess(self),
                "mouse_trace" => analyse_mouse_trace(self),
                "vision_transfer" => analyse_vision_transfer(self),
                "survive_poss" => analyse_survive_poss(self),
                "super_fl_local" => analyse_super_fl_local(self),
                _ => continue,
            };
        }
    }
    pub fn print_event(&self) {
        let mut num = 0;
        for e in &self.video_action_state_recorder {
            // if num < 800 {
            //     if e.mouse != "mv" {
            //         println!(
            //             "time = {:?}, mouse = {:?}, x = {:?}, y = {:?}, level = {:?}",
            //             e.time, e.mouse, e.x, e.y, e.useful_level
            //         );
            //     }
            // }
            // println!(
            //     "time = {:?}, mouse = {:?}, x = {:?}, y = {:?}, level = {:?}",
            //     e.time, e.mouse, e.x, e.y, e.useful_level
            // );
            if e.mouse != "mmv" {
                println!(
                    "time = {:?}, mouse = {:?}, x = {:?}, y = {:?}, level = {:?}",
                    e.time, e.mouse, e.x, e.y, e.useful_level
                );
            }
            num += 1;
            // if e.mouse != "mv" {
            //     println!(
            //         "time = {:?}, mouse = {:?}, x = {:?}, y = {:?}",
            //         e.time, e.mouse, e.x, e.y
            //     );
            //     e.prior_game_board
            //         .game_board
            //         .iter()
            //         .for_each(|v| println!("{:?}", v));
            //     e.prior_game_board
            //         .poss
            //         .iter()
            //         .for_each(|v| println!("{:?}", v));
            // }
        }
    }
    pub fn print_comments(&self) {
        for i in &self.video_action_state_recorder {
            if !i.comments.is_empty() {
                println!("{:?} => {:?}", i.time, i.comments);
            }
        }
    }
    pub fn print_raw_data(&self, n: usize) {
        for i in 0..n {
            let v = self.raw_data[i];
            print!("{:?}", v as char);
        }
        println!();
        for i in 0..n {
            let v = self.raw_data[i];
            print!("{:?}, ", v);
        }
    }
}
impl BaseVideo {
    // 再实现一些get、set方法
    pub fn set_pix_size(&mut self, pix_size: u8) {
        if self.game_board_state != GameBoardState::Ready
            && self.game_board_state != GameBoardState::Win
            && self.game_board_state != GameBoardState::Loss
        {
            return;
        }
        self.cell_pixel_size = pix_size;
    }
    /// 获取当前录像时刻的后验的游戏局面
    pub fn get_game_board(&self) -> Vec<Vec<i32>> {
        if self.game_board_state == GameBoardState::Display {
            return self.game_board_stream[self.video_action_state_recorder[self.current_event_id]
                .next_game_board_id as usize]
                .game_board
                .clone();
        } else {
            return self.minesweeper_board.game_board.clone();
        }
    }
    /// 获取当前录像时刻的局面概率
    pub fn get_game_board_poss(&mut self) -> Vec<Vec<f64>> {
        let mut id = self.current_event_id;
        loop {
            if self.video_action_state_recorder[id].useful_level < 2 {
                id -= 1;
                if id <= 0 {
                    let p = self.mine_num as f64 / (self.height * self.width) as f64;
                    return vec![vec![p; self.height]; self.width];
                }
            } else {
                // println!("{:?}, {:?}",self.current_event_id, self.video_action_state_recorder.len());
                return self.game_board_stream[self.video_action_state_recorder
                    [self.current_event_id]
                    .next_game_board_id as usize]
                    .get_poss()
                    .to_vec();
                // return self.events[id].prior_game_board.get_poss().clone();
            }
        }
    }
    // 录像解析时，设置游戏时间，时间成绩。
    pub fn set_rtime(&mut self, time: f64) -> Result<u8, ()> {
        if !self.allow_set_rtime {
            return Err(());
        }
        self.game_dynamic_params.rtime = time;
        self.game_dynamic_params.rtime_ms = s_to_ms(time);
        self.allow_set_rtime = false;
        Ok(0)
    }
    /// 用于计数器上显示的时间,和arbiter一致
    pub fn get_time(&self) -> f64 {
        match self.game_board_state {
            GameBoardState::Playing => {
                let now = Instant::now();
                return now.duration_since(self.game_start_instant).as_millis() as f64 / 1000.0;
            }
            GameBoardState::PreFlaging => {
                let now = Instant::now();
                return now.duration_since(self.video_start_instant).as_millis() as f64 / 1000.0;
            }
            GameBoardState::Loss | GameBoardState::Win => self.game_dynamic_params.rtime,
            GameBoardState::Ready => 0.0,
            GameBoardState::Display => self.current_time,
        }
    }
    pub fn get_rtime(&self) -> Result<f64, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
            && self.game_board_state != GameBoardState::Display
        {
            return Err(());
        }
        Ok(self.game_dynamic_params.rtime)
    }
    pub fn get_rtime_ms(&self) -> Result<u32, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
            && self.game_board_state != GameBoardState::Display
        {
            return Err(());
        }
        Ok(self.game_dynamic_params.rtime_ms)
    }
    /// video_time是录像的总时长。录像市场比时间成绩多一部分。
    pub fn get_video_time(&self) -> Result<f64, ()> {
        if self.game_board_state != GameBoardState::Display {
            return Err(());
        }
        Ok(self.video_action_state_recorder.last().unwrap().time)
        // Ok(self.game_dynamic_params.rtime + self.delta_time)
    }
    /// 录像播放时，按时间设置current_time；超出两端范围取两端。
    /// 游戏时不要调用。
    pub fn set_current_time(&mut self, time: f64) {
        if time > self.video_action_state_recorder[self.current_event_id].time {
            loop {
                if self.current_event_id >= self.video_action_state_recorder.len() - 1 {
                    // 最后一帧
                    break;
                }
                self.current_event_id += 1;
                if self.video_action_state_recorder[self.current_event_id].time <= time {
                    continue;
                } else {
                    self.current_event_id -= 1;
                    break;
                }
            }
        } else {
            loop {
                if self.current_event_id == 0 {
                    break;
                }
                self.current_event_id -= 1;
                if self.video_action_state_recorder[self.current_event_id].time > time {
                    continue;
                } else {
                    break;
                }
            }
        }
        self.current_time = self.video_action_state_recorder[self.current_event_id].time;
    }
    /// 设置current_event_id
    pub fn set_current_event_id(&mut self, id: usize) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Display {
            return Err(());
        };
        self.current_event_id = id;
        self.current_time = self.video_action_state_recorder[id].time;
        Ok(0)
    }
    pub fn set_is_offical(&mut self, is_offical: bool) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.is_offical = is_offical;
        Ok(0)
    }
    pub fn set_is_fair(&mut self, is_fair: bool) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.is_fair = is_fair;
        Ok(0)
    }
    pub fn set_mode(&mut self, mode: u16) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.mode = mode;
        Ok(0)
    }
    pub fn set_software(&mut self, software: Vec<u8>) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
            && self.game_board_state != GameBoardState::Ready
        {
            return Err(());
        };
        self.software = software;
        Ok(0)
    }
    /// 两种情况调用：游戏开始前、可猜模式（尺寸相等，雷数不检查）
    pub fn set_board(&mut self, board: Vec<Vec<i32>>) -> Result<u8, ()> {
        match self.game_board_state {
            GameBoardState::Playing | GameBoardState::Ready | GameBoardState::PreFlaging => {
                if self.width != board[0].len() || self.height != board.len() {
                    return Err(());
                }
            }
            GameBoardState::Display | GameBoardState::Win | GameBoardState::Loss => return Err(()),
        }
        self.mine_num = board.iter().fold(0, |y, row| {
            y + row
                .iter()
                .fold(0, |yy, x| if *x == -1 { yy + 1 } else { yy })
        });
        self.board = board.clone();
        self.minesweeper_board.board = board;
        Ok(0)
    }
    pub fn set_player_designator(&mut self, player_designator: Vec<u8>) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.player_designator = player_designator;
        Ok(0)
    }
    pub fn set_race_designator(&mut self, race_designator: Vec<u8>) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.race_designator = race_designator;
        Ok(0)
    }
    pub fn set_uniqueness_designator(&mut self, uniqueness_designator: Vec<u8>) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.uniqueness_designator = uniqueness_designator;
        Ok(0)
    }
    /// 拟弃用，会自动记录
    pub fn set_start_time(&mut self, start_time: Vec<u8>) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.start_time = start_time;
        Ok(0)
    }
    /// 拟弃用，会自动记录
    pub fn set_end_time(&mut self, end_time: Vec<u8>) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.end_time = end_time;
        Ok(0)
    }
    pub fn set_country(&mut self, country: Vec<u8>) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.country = country;
        Ok(0)
    }
    pub fn set_checksum(&mut self, checksum: [u8; 32]) -> Result<u8, ()> {
        if self.game_board_state != GameBoardState::Loss
            && self.game_board_state != GameBoardState::Win
        {
            return Err(());
        };
        self.checksum = checksum;
        Ok(0)
    }
    pub fn get_left(&self) -> usize {
        match self.game_board_state {
            GameBoardState::Display => {
                self.video_action_state_recorder[self.current_event_id]
                    .key_dynamic_params
                    .left
            }
            _ => self.minesweeper_board.left,
        }
    }
    pub fn get_right(&self) -> usize {
        match self.game_board_state {
            GameBoardState::Display => {
                self.video_action_state_recorder[self.current_event_id]
                    .key_dynamic_params
                    .right
            }
            _ => self.minesweeper_board.right,
        }
    }
    pub fn get_double(&self) -> usize {
        match self.game_board_state {
            GameBoardState::Display => {
                self.video_action_state_recorder[self.current_event_id]
                    .key_dynamic_params
                    .double
            }
            _ => self.minesweeper_board.double,
        }
    }
    pub fn get_cl(&self) -> usize {
        self.get_left() + self.get_right() + self.get_double()
    }
    pub fn get_flag(&self) -> usize {
        match self.game_board_state {
            GameBoardState::Display => {
                self.video_action_state_recorder[self.current_event_id]
                    .key_dynamic_params
                    .flag
            }
            _ => self.minesweeper_board.flag,
        }
    }
    pub fn get_left_s(&self) -> f64 {
        match self.game_board_state {
            GameBoardState::Display => {
                if self.current_time < 0.00099 {
                    return 0.0;
                }
                self.get_left() as f64 / self.current_time
            }
            GameBoardState::Loss | GameBoardState::Win => self.game_dynamic_params.left_s,
            GameBoardState::PreFlaging | GameBoardState::Ready => 0.0,
            GameBoardState::Playing => {
                let now = Instant::now();
                let t_ms = now.duration_since(self.game_start_instant).as_millis() as f64;
                self.get_left() as f64 * 1000.0 / t_ms
            }
        }
    }
    pub fn get_right_s(&self) -> f64 {
        match self.game_board_state {
            GameBoardState::Display => {
                if self.current_time < 0.00099 {
                    return 0.0;
                }
                self.get_right() as f64 / self.current_time
            }
            GameBoardState::Loss | GameBoardState::Win => self.game_dynamic_params.right_s,
            GameBoardState::PreFlaging | GameBoardState::Ready => 0.0,
            GameBoardState::Playing => {
                let now = Instant::now();
                let t_ms = now.duration_since(self.game_start_instant).as_millis() as f64;
                self.get_right() as f64 * 1000.0 / t_ms
            }
        }
    }
    pub fn get_double_s(&self) -> f64 {
        match self.game_board_state {
            GameBoardState::Display => {
                if self.current_time < 0.00099 {
                    return 0.0;
                }
                self.get_double() as f64 / self.current_time
            }
            GameBoardState::Loss | GameBoardState::Win => self.game_dynamic_params.double_s,
            GameBoardState::PreFlaging | GameBoardState::Ready => 0.0,
            GameBoardState::Playing => {
                let now = Instant::now();
                let t_ms = now.duration_since(self.game_start_instant).as_millis() as f64;
                self.get_double() as f64 * 1000.0 / t_ms
            }
        }
    }
    pub fn get_cl_s(&self) -> f64 {
        match self.game_board_state {
            GameBoardState::Display => {
                if self.current_time < 0.00099 {
                    return 0.0;
                }
                self.get_cl() as f64 / self.current_time
            }
            GameBoardState::Loss | GameBoardState::Win => self.game_dynamic_params.cl_s,
            GameBoardState::PreFlaging | GameBoardState::Ready => 0.0,
            GameBoardState::Playing => {
                let now = Instant::now();
                let t_ms = now.duration_since(self.game_start_instant).as_millis() as f64;
                self.get_cl() as f64 * 1000.0 / t_ms
            }
        }
    }
    pub fn get_flag_s(&self) -> f64 {
        match self.game_board_state {
            GameBoardState::Display => {
                if self.current_time < 0.00099 {
                    return 0.0;
                }
                self.get_flag() as f64 / self.current_time
            }
            GameBoardState::Loss | GameBoardState::Win => self.game_dynamic_params.flag_s,
            GameBoardState::PreFlaging | GameBoardState::Ready => 0.0,
            GameBoardState::Playing => {
                let now = Instant::now();
                let t_ms = now.duration_since(self.game_start_instant).as_millis() as f64;
                self.get_flag() as f64 * 1000.0 / t_ms
            }
        }
    }
    pub fn get_path(&self) -> f64 {
        if self.video_action_state_recorder.is_empty() {
            return 0.0;
        }
        if self.game_board_state == GameBoardState::Display {
            self.video_action_state_recorder[self.current_event_id].path
        } else {
            self.video_action_state_recorder.last().unwrap().path
        }
    }
    pub fn get_etime(&self) -> Result<f64, ()> {
        let bbbv_solved = self.get_bbbv_solved()?;
        if bbbv_solved == 0 {
            return Ok(0.0);
        }
        if self.game_board_state == GameBoardState::Display {
            let t = self.video_action_state_recorder[self.current_event_id].time - self.delta_time;
            Ok(t / bbbv_solved as f64 * self.static_params.bbbv as f64)
        } else {
            let t = self.game_dynamic_params.rtime;
            Ok(t / bbbv_solved as f64 * self.static_params.bbbv as f64)
        }
    }
    pub fn get_bbbv_s(&self) -> Result<f64, ()> {
        let bbbv_solved = self.get_bbbv_solved()?;
        if self.game_board_state == GameBoardState::Display {
            if self.current_time < 0.00099 {
                return Ok(0.0);
            }
            return Ok(bbbv_solved as f64 / self.current_time);
        }
        Ok(bbbv_solved as f64 / self.game_dynamic_params.rtime)
    }
    pub fn get_bbbv_solved(&self) -> Result<usize, ()> {
        match self.game_board_state {
            GameBoardState::Display => Ok(self.video_action_state_recorder[self.current_event_id]
                .key_dynamic_params
                .bbbv_solved),
            // GameBoardState::Win | GameBoardState::Loss => Ok(self
            //     .video_dynamic_params
            //     .bbbv_solved),
            GameBoardState::Win | GameBoardState::Loss => Ok(self
                .video_action_state_recorder
                .last()
                .unwrap()
                .key_dynamic_params
                .bbbv_solved),
            _ => Err(()),
        }
    }
    pub fn get_stnb(&self) -> Result<f64, ()> {
        let bbbv_solved = self.get_bbbv_solved()?;
        // println!("self.current_time:{:?}", bbbv_solved);
        if self.game_board_state == GameBoardState::Display && self.current_time < 0.00099 {
            return Ok(0.0);
        }
        let c;
        match (self.height, self.width, self.mine_num) {
            (8, 8, 10) => c = 47.299,
            (16, 16, 40) => c = 153.73,
            (16, 30, 99) => c = 435.001,
            _ => return Ok(0.0),
        }

        if self.game_board_state == GameBoardState::Display {
            let t = self.video_action_state_recorder[self.current_event_id].time - self.delta_time;
            Ok(c * self.static_params.bbbv as f64 / t.powf(1.7)
                * (bbbv_solved as f64 / self.static_params.bbbv as f64).powf(0.5))
        } else {
            Ok(
                c * self.static_params.bbbv as f64 / self.game_dynamic_params.rtime.powf(1.7)
                    * ((bbbv_solved as f64 / self.static_params.bbbv as f64).powf(0.5)),
            )
        }
    }
    pub fn get_rqp(&self) -> Result<f64, ()> {
        let bbbv_solved = self.get_bbbv_solved()?;
        if bbbv_solved == 0 {
            return Ok(0.0);
        }
        Ok(self.current_time.powf(2.0) / bbbv_solved as f64)
    }
    pub fn get_qg(&self) -> Result<f64, ()> {
        let bbbv_solved = self.get_bbbv_solved()?;
        if bbbv_solved == 0 {
            return Ok(0.0);
        }
        Ok(self.current_time.powf(1.7) / bbbv_solved as f64)
    }
    pub fn get_ce(&self) -> Result<usize, ()> {
        match self.game_board_state {
            GameBoardState::Display => Ok(self.video_action_state_recorder[self.current_event_id]
                .key_dynamic_params
                .ce),
            GameBoardState::Win | GameBoardState::Loss => Ok(self
                .video_action_state_recorder
                .last()
                .unwrap()
                .key_dynamic_params
                .ce),
            _ => Err(()),
        }
    }
    pub fn get_ce_s(&self) -> Result<f64, ()> {
        let ce = self.get_ce()?;
        if self.current_time < 0.00099 {
            return Ok(0.0);
        }
        Ok(ce as f64 / self.current_time)
    }
    pub fn get_corr(&self) -> Result<f64, ()> {
        let ce = self.get_ce()?;
        let cl = self.get_cl();
        if cl == 0 {
            return Ok(0.0);
        }
        Ok(ce as f64 / cl as f64)
    }
    pub fn get_thrp(&self) -> Result<f64, ()> {
        let ce = self.get_ce()?;
        let bbbv_solved = self.get_bbbv_solved().unwrap();
        if ce == 0 {
            return Ok(0.0);
        }
        Ok(bbbv_solved as f64 / ce as f64)
    }
    pub fn get_ioe(&self) -> Result<f64, ()> {
        let bbbv_solved = self.get_bbbv_solved()?;
        let cl = self.get_cl();
        if cl == 0 {
            return Ok(0.0);
        }
        Ok(bbbv_solved as f64 / cl as f64)
    }
    pub fn get_op_solved(&self) -> Result<usize, ()> {
        if self.game_board_state != GameBoardState::Display
            && self.game_board_state != GameBoardState::Win
            && self.game_board_state != GameBoardState::Loss
        {
            return Err(());
        };
        Ok(self.video_action_state_recorder[self.current_event_id]
            .key_dynamic_params
            .op_solved)
    }
    pub fn get_isl_solved(&self) -> Result<usize, ()> {
        if self.game_board_state != GameBoardState::Display
            && self.game_board_state != GameBoardState::Win
            && self.game_board_state != GameBoardState::Loss
        {
            return Err(());
        };
        Ok(self.video_action_state_recorder[self.current_event_id]
            .key_dynamic_params
            .isl_solved)
    }
    /// 跨语言调用时，不能传递枚举体用这个
    pub fn get_mouse_state(&self) -> usize {
        let m_s;
        if self.game_board_state == GameBoardState::Display {
            m_s = self.video_action_state_recorder[self.current_event_id].mouse_state;
        } else {
            m_s = self.minesweeper_board.mouse_state;
        }
        match m_s {
            MouseState::UpUp => 1,
            MouseState::UpDown => 2,
            MouseState::UpDownNotFlag => 3,
            MouseState::DownUp => 4,
            MouseState::Chording => 5,
            MouseState::ChordingNotFlag => 6,
            MouseState::DownUpAfterChording => 7,
            MouseState::Undefined => 8,
        }
    }
    // 录像播放时，返回鼠标的坐标
    pub fn get_x_y(&self) -> Result<(u16, u16), ()> {
        if self.game_board_state != GameBoardState::Display {
            return Err(());
        };
        Ok((
            (self.video_action_state_recorder[self.current_event_id].x as f64
                * self.video_playing_pix_size_k) as u16,
            (self.video_action_state_recorder[self.current_event_id].y as f64
                * self.video_playing_pix_size_k) as u16,
        ))
    }
    // 录像播放时，设置按何种像素播放，涉及鼠标位置回报
    pub fn set_video_playing_pix_size(&mut self, pix_size: u8) {
        self.video_playing_pix_size_k = pix_size as f64 / self.cell_pixel_size as f64;
    }
}

impl BaseVideo {
    /// 按evf标准，编码出原始二进制数据
    pub fn generate_evf_v0_raw_data(&mut self) {
        self.raw_data = vec![0, 0];
        if self.is_completed {
            self.raw_data[1] |= 0b1000_0000;
        }
        if self.is_offical {
            self.raw_data[1] |= 0b0100_0000;
        }
        if self.is_fair {
            self.raw_data[1] |= 0b0010_0000;
        }
        self.raw_data.push(self.height as u8);
        self.raw_data.push(self.width as u8);
        self.raw_data.push((self.mine_num >> 8).try_into().unwrap());
        self.raw_data
            .push((self.mine_num % 256).try_into().unwrap());
        self.raw_data.push(self.cell_pixel_size);
        self.raw_data.push((self.mode >> 8).try_into().unwrap());
        self.raw_data.push((self.mode % 256).try_into().unwrap());
        self.raw_data
            .push((self.static_params.bbbv >> 8).try_into().unwrap());
        self.raw_data
            .push((self.static_params.bbbv % 256).try_into().unwrap());
        self.raw_data.push(
            (self.game_dynamic_params.rtime_ms >> 16)
                .try_into()
                .unwrap(),
        );
        self.raw_data.push(
            ((self.game_dynamic_params.rtime_ms >> 8) % 256)
                .try_into()
                .unwrap(),
        );
        self.raw_data.push(
            (self.game_dynamic_params.rtime_ms % 256)
                .try_into()
                .unwrap(),
        );
        self.raw_data.append(&mut self.software.clone().to_owned());
        self.raw_data.push(0);
        self.raw_data
            .append(&mut self.player_designator.clone().to_owned());
        self.raw_data.push(0);
        self.raw_data
            .append(&mut self.race_designator.clone().to_owned());
        self.raw_data.push(0);
        self.raw_data
            .append(&mut self.uniqueness_designator.clone().to_owned());
        self.raw_data.push(0);
        self.raw_data
            .append(&mut self.start_time.clone().to_owned());
        self.raw_data.push(0);
        self.raw_data.append(&mut self.end_time.clone().to_owned());
        self.raw_data.push(0);
        self.raw_data.append(&mut self.country.clone().to_owned());
        self.raw_data.push(0);

        let mut byte = 0;
        let mut ptr = 0;
        for i in 0..self.height {
            for j in 0..self.width {
                byte <<= 1;
                if self.board[i][j] == -1 {
                    byte |= 1;
                }
                ptr += 1;
                if ptr == 8 {
                    self.raw_data.push(byte);
                    ptr = 0;
                    byte = 0;
                }
            }
        }
        if ptr > 0 {
            byte <<= 8 - ptr;
            self.raw_data.push(byte);
        }

        for event in &self.video_action_state_recorder {
            // println!("{:?}: '{:?}', ({:?}, {:?})", event.time, event.mouse.as_str(), event.x, event.y);
            match event.mouse.as_str() {
                "mv" => self.raw_data.push(1),
                "lc" => self.raw_data.push(2),
                "lr" => self.raw_data.push(3),
                "rc" => self.raw_data.push(4),
                "rr" => self.raw_data.push(5),
                "mc" => self.raw_data.push(6),
                "mr" => self.raw_data.push(7),
                "pf" => self.raw_data.push(8),
                "cc" => self.raw_data.push(9),
                // 不可能出现，出现再说
                _ => self.raw_data.push(99),
            }
            let t_ms = s_to_ms(event.time);
            self.raw_data.push((t_ms >> 16).try_into().unwrap());
            self.raw_data.push(((t_ms >> 8) % 256).try_into().unwrap());
            self.raw_data.push((t_ms % 256).try_into().unwrap());
            self.raw_data.push((event.x >> 8).try_into().unwrap());
            self.raw_data.push((event.x % 256).try_into().unwrap());
            self.raw_data.push((event.y >> 8).try_into().unwrap());
            self.raw_data.push((event.y % 256).try_into().unwrap());
        }
        if !self.checksum.is_empty() {
            self.raw_data.push(0);
            self.raw_data
                .append(&mut self.checksum.clone().to_vec().to_owned());
        } else {
            self.raw_data.push(255);
        }
    }
    /// 存evf文件，自动加后缀，xxx.evf重复变成xxx(2).evf
    pub fn save_to_evf_file(&self, file_name: &str) {
        let file_exist =
            std::path::Path::new((file_name.to_string() + &(".evf".to_string())).as_str()).exists();
        if !file_exist {
            fs::write(
                (file_name.to_string() + &(".evf".to_string())).as_str(),
                &self.raw_data,
            )
            .unwrap();
            return;
        } else {
            let mut id = 2;
            let mut format_name;
            loop {
                format_name = file_name.to_string() + &(format!("({}).evf", id).to_string());
                let new_file_name = format_name.as_str();
                let file_exist = std::path::Path::new(new_file_name).exists();
                if !file_exist {
                    fs::write(new_file_name, &self.raw_data).unwrap();
                    return;
                }
                id += 1;
            }
        }
    }
}
