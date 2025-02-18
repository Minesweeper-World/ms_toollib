// use crate::MouseState;
// use crate::miscellaneous::s_to_ms;
// use crate::utils::{cal_board_numbers};
use std::cmp::{max, min};
use crate::videos::base_video::{BaseVideo, ErrReadVideoReason, VideoActionStateRecorder};


/// avf录像解析器。  
/// - 功能：解析avf格式的录像，有详细分析录像的方法。  
/// - 以下是在python中调用的示例。  
/// ```python
/// v = ms.AvfVideo("video_name.avf") # 第一步，读取文件的二进制内容
/// v.parse_video() # 第二步，解析文件的二进制内容
/// v.analyse() # 第三步，根据解析到的内容，推衍整个局面
/// print(v.bbbv)
/// print(v.clicks)
/// print(v.clicks_s)
/// print("对象上的所有属性和方法：" + dir(v))
/// v.analyse_for_features(["high_risk_guess"]) # 用哪些分析方法。分析结果会记录到events.comments里
/// for i in range(v.events_len):
///     print(v.events_time(i), v.events_x(i), v.events_y(i), v.events_mouse(i))
/// for i in range(v.events_len):
///     if v.events_useful_level(i) >= 2:
///         print(v.events_posteriori_game_board(i).poss)
/// ```
pub struct AvfVideo {
    pub file_name: String,
    pub data: BaseVideo,
}

impl AvfVideo {
    #[cfg(any(feature = "py", feature = "rs"))]
    pub fn new(file_name: &str) -> AvfVideo {
        AvfVideo {
            file_name: file_name.to_string(),
            data: BaseVideo::new_with_file(file_name),
        }
    }
    #[cfg(feature = "js")]
    pub fn new(video_data: Vec<u8>) -> AvfVideo {
        AvfVideo {
            file_name: file_name.to_string(),
            data: BaseVideo::new(video_data),
        }
    }
    pub fn parse_video(&mut self) -> Result<(), ErrReadVideoReason> {
        match self.data.get_u8() {
            Ok(_) => {}
            Err(_) => return Err(ErrReadVideoReason::FileIsEmpty),
        };
        self.data.offset += 4;
        self.data.level = self.data.get_u8()?;
        // println!("{:?}", self.data.level);
        match self.data.level {
            3 => {
                self.data.width = 8;
                self.data.height = 8;
                self.data.mine_num = 10;
            }
            4 => {
                self.data.width = 16;
                self.data.height = 16;
                self.data.mine_num = 40;
            }
            5 => {
                self.data.width = 30;
                self.data.height = 16;
                self.data.mine_num = 99;
            }
            6 => {
                self.data.width = self.data.get_u8()? as usize + 1;
                self.data.height = self.data.get_u8()? as usize + 1;
                self.data.mine_num = self.data.get_u16()? as usize;
            }
            _ => return Err(ErrReadVideoReason::InvalidLevel),
        }
        self.data.board = vec![vec![0; self.data.width]; self.data.height];
        for _ in 0..self.data.mine_num {
            let c = self.data.get_u8()? as usize;
            let d = self.data.get_u8()? as usize;
            self.data.board[c - 1][d - 1] = -1;
        }

        for x in 0..self.data.height {
            for y in 0..self.data.width {
                if self.data.board[x][y] == -1 {
                    for j in max(1, x) - 1..min(self.data.height, x + 2) {
                        for k in max(1, y) - 1..min(self.data.width, y + 2) {
                            if self.data.board[j][k] >= 0 {
                                self.data.board[j][k] += 1;
                            }
                        }
                    }
                }
            }
        } // 算数字
        let mut buffer: [char; 3] = ['\0', '\0', '\0'];
        loop {
            buffer[0] = buffer[1];
            buffer[1] = buffer[2];
            buffer[2] = self.data.get_char()?;
            if buffer[0] == '['
                && (buffer[1] == '0' || buffer[1] == '1' || buffer[1] == '2' || buffer[1] == '3')
                && buffer[2] == '|'
            {
                break;
            }
        }
        loop {
            let v = self.data.get_char()?;
            match v {
                '|' => break,
                _ => self.data.start_time.push(v as u8),
            }
        }
        // println!("666");
        // loop {
        //     let v = self.get_char()?;
        //     print!("{:?}", v as char);
        // }
        loop {
            let v = self.data.get_char()?;
            match v {
                '|' => break,
                _ => self.data.end_time.push(v as u8),
            }
        }
        let v = self.data.get_char()?;
        let mut buffer: [char; 2];
        match v {
            '|' => buffer = ['\0', '|'],
            'B' => buffer = ['|', 'B'],
            _ => buffer = ['\0', '\0'],
        }
        // 此处以下10行的写法有危险
        loop {
            if buffer[0] == '|' && buffer[1] == 'B' {
                break;
            }
            buffer[0] = buffer[1];
            buffer[1] = self.data.get_char()?;
        }
        let mut s: String = "".to_string();
        loop {
            let v = self.data.get_char()?;
            match v {
                'T' => break,
                _ => s.push(v),
            }
        }
        self.data.static_params.bbbv = match s.parse() {
            Ok(v) => v,
            Err(_) => return Err(ErrReadVideoReason::InvalidParams),
        };
        let mut s: String = "".to_string();
        loop {
            let v = self.data.get_char()?;
            match v {
                ']' => break,
                _ => s.push(v),
            }
        }
        s = str::replace(&s, ",", "."); // 有些录像小数点是逗号
        match s.parse::<f64>() {
            Ok(v) => self.data.set_rtime(v).unwrap(),
            Err(_) => return Err(ErrReadVideoReason::InvalidParams),
        };
        let mut buffer = [0u8; 8];
        while buffer[2] != 1 || buffer[1] > 1 {
            buffer[0] = buffer[1];
            buffer[1] = buffer[2];
            buffer[2] = self.data.get_u8()?;
        }
        for i in 3..8 {
            buffer[i] = self.data.get_u8()?;
        }
        loop {
            // if buffer[0] != 1 {
            // println!("{:?}, {:?}", ((buffer[6] as u16) << 8 | buffer[2] as u16) as f64 - 1.0
            // + (buffer[4] as f64) / 100.0, buffer[0]);}
            self.data.video_action_state_recorder.push(VideoActionStateRecorder {
                time: ((buffer[6] as u16) << 8 | buffer[2] as u16) as f64 - 1.0
                    + (buffer[4] as f64) / 100.0,
                mouse: match buffer[0] {
                    1 => "mv".to_string(),
                    3 => "lc".to_string(),
                    5 => "lr".to_string(),
                    9 => "rc".to_string(),
                    17 => "rr".to_string(),
                    33 => "mc".to_string(),
                    65 => "mr".to_string(),
                    145 => "rr".to_string(),
                    193 => "mr".to_string(),
                    11 => "sc".to_string(),
                    21 => "lr".to_string(),
                    _ => return Err(ErrReadVideoReason::InvalidVideoEvent),
                },
                // column: 0,
                // row: 0,
                x: (buffer[1] as u16) << 8 | buffer[3] as u16,
                y: (buffer[5] as u16) << 8 | buffer[7] as u16,
                ..VideoActionStateRecorder::default()
            });
            for i in 0..8 {
                // ???????
                buffer[i] = self.data.get_u8()?;
            }
            if buffer[2] == 0 && buffer[6] == 0 {
                break;
            }
        }
        // 标识符
        while self.data.get_char()? != 'S' {}
        while self.data.get_char()? != 'k' {}
        while self.data.get_char()? != 'i' {}
        while self.data.get_char()? != 'n' {}
        while self.data.get_char()? != ':' {}
        while self.data.get_char()? != '\r' {}
        loop {
            let v = self.data.get_char()?;
            match v {
                '\r' => break,
                _ => self.data.player_designator.push(v as u8),
            }
        }
        self.data.software = "Arbiter".as_bytes().to_vec();
        // for i in 0..1000 {
        //     for j in 0..8 {
        //         print!("{:?},", self.get_char().unwrap() as u8);
        //     }
        //     println!("");
        // }
        self.data.can_analyse = true;
        Ok(())
    }
}

