use serde::{Deserialize, Serialize};

/// 计时状态（PRD 7.1）。所有状态转换只在 Rust 侧发生。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// 未启动
    Idle,
    /// 工作计时中
    Working,
    /// 已发出提醒，等待用户响应
    Reminding,
    /// 活动中
    Break,
    /// 用户主动暂停
    Paused,
    /// 系统状态导致挂起（锁屏 / 休眠，Phase 3 接入）
    Suspended,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Working => "working",
            Phase::Reminding => "reminding",
            Phase::Break => "break",
            Phase::Paused => "paused",
            Phase::Suspended => "suspended",
        }
    }
}

/// 状态机运行所需的最小计划快照，避免核心逻辑依赖数据库结构。
#[derive(Debug, Clone)]
pub struct PlanSpec {
    pub work_seconds: u64,
    pub break_seconds: u64,
    pub completion_mode: String,
    pub repeat_times: u32,
    pub repeat_interval_seconds: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Machine {
    pub phase: Phase,
    pub plan: Option<PlanSpec>,
    /// 当前周期累计的有效工作时长（秒）
    pub work_seconds: u64,
    /// 当前活动已进行的秒数
    pub break_seconds: u64,
    pub break_target: u64,
    pub activity_id: Option<String>,
    pub activity_name: Option<String>,
    pub activity_category: Option<String>,
    pub cycle_index: u64,
    pub ignored_count: u32,
    /// 提醒已持续未响应的秒数
    pub reminder_elapsed: u64,
    /// 稍后提醒的剩余秒数
    pub snooze_remaining: Option<u64>,
    /// 勿扰模式剩余秒数
    pub dnd_remaining: Option<u64>,
    /// 被挂起前所处的状态
    pub suspended_from: Option<Phase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// 达到阈值，需要真正发出一条提醒
    Reminder,
    /// 稍后提醒的时间到了，需要再次提醒
    ReminderRepeated,
    /// 活动倒计时自然结束
    BreakElapsed,
    /// 进入下一个工作周期
    NextCycle,
}

impl Default for Phase {
    fn default() -> Self {
        Phase::Idle
    }
}

impl Machine {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            ..Default::default()
        }
    }

    pub fn set_plan(&mut self, plan: PlanSpec) {
        self.plan = Some(plan);
    }

    pub fn work_target(&self) -> u64 {
        self.plan.as_ref().map(|p| p.work_seconds).unwrap_or(0)
    }

    pub fn break_target_seconds(&self) -> u64 {
        self.break_target
    }

    pub fn remaining_work_seconds(&self) -> u64 {
        self.work_target().saturating_sub(self.work_seconds)
    }

    pub fn remaining_break_seconds(&self) -> u64 {
        self.break_target.saturating_sub(self.break_seconds)
    }

    /// 启动计时：进入工作状态并从零开始（若已有进度则从当前进度继续）。
    pub fn start(&mut self) -> Result<(), String> {
        if self.plan.is_none() {
            return Err("尚未选择提醒计划".into());
        }
        if self.phase == Phase::Working {
            return Ok(());
        }
        if self.phase == Phase::Break {
            return Err("活动进行中，不能开始新的工作计时".into());
        }
        self.phase = Phase::Working;
        self.suspended_from = None;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), String> {
        match self.phase {
            Phase::Working | Phase::Reminding => {
                self.suspended_from = None;
                self.phase = Phase::Paused;
                self.snooze_remaining = None;
                Ok(())
            }
            Phase::Suspended => {
                // 挂起状态（离开座位 / 非工作时间）下也可以直接暂停，切到「用户主动暂停」
                self.suspended_from = None;
                self.phase = Phase::Paused;
                Ok(())
            }
            Phase::Break => Err("活动进行中，请先结束活动再暂停提醒".into()),
            _ => Err("当前不在计时状态，无法暂停".into()),
        }
    }

    pub fn resume(&mut self) -> Result<(), String> {
        if self.phase != Phase::Paused && self.phase != Phase::Suspended {
            return Err("当前不是暂停 / 挂起状态，无需恢复".into());
        }
        if self.plan.is_none() {
            return Err("尚未选择提醒计划".into());
        }
        self.phase = Phase::Working;
        self.suspended_from = None;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.phase = Phase::Idle;
        self.work_seconds = 0;
        self.break_seconds = 0;
        self.break_target = 0;
        self.activity_id = None;
        self.activity_name = None;
        self.activity_category = None;
        self.cycle_index = 0;
        self.reminder_elapsed = 0;
        self.snooze_remaining = None;
        self.suspended_from = None;
        self.ignored_count = 0;
    }

    /// 挂起：离开座位（长时间无键鼠输入）或不在工作时间时调用，计时停止累加。
    pub fn suspend(&mut self) -> Result<(), String> {
        match self.phase {
            Phase::Working | Phase::Reminding => {
                self.suspended_from = Some(self.phase);
                self.phase = Phase::Suspended;
                Ok(())
            }
            _ => Err("当前状态无需挂起".into()),
        }
    }

    /// 用户回到座位 / 回到工作时间，恢复挂起前的状态。
    pub fn restore_from_suspend(&mut self) -> Result<(), String> {
        if self.phase != Phase::Suspended {
            return Err("当前不是挂起状态".into());
        }
        self.phase = self.suspended_from.unwrap_or(Phase::Working);
        self.suspended_from = None;
        Ok(())
    }

    /// 工作计时推进，返回需要引擎处理的触发事件。
    pub fn tick_work(&mut self, seconds: u64) -> Option<Trigger> {
        match self.phase {
            Phase::Working => {
                if self.plan.is_none() {
                    return None;
                }
                if self.snooze_remaining.is_some() {
                    return self.tick_snooze(seconds);
                }
                self.work_seconds = self.work_seconds.saturating_add(seconds);
                if self.work_seconds >= self.work_target() {
                    self.phase = Phase::Reminding;
                    self.reminder_elapsed = 0;
                    Some(Trigger::Reminder)
                } else {
                    None
                }
            }
            Phase::Reminding => self.tick_snooze(seconds),
            Phase::Break => self.tick_break(seconds),
            _ => None,
        }
    }

    fn tick_snooze(&mut self, seconds: u64) -> Option<Trigger> {
        if let Some(remaining) = self.snooze_remaining {
            let left = remaining.saturating_sub(seconds);
            self.snooze_remaining = if left == 0 { None } else { Some(left) };
            if left == 0 {
                self.phase = Phase::Reminding;
                self.reminder_elapsed = 0;
                return Some(Trigger::ReminderRepeated);
            }
            return None;
        }
        self.reminder_elapsed = self.reminder_elapsed.saturating_add(seconds);
        None
    }

    fn tick_break(&mut self, seconds: u64) -> Option<Trigger> {
        self.break_seconds = self.break_seconds.saturating_add(seconds);
        if self.break_seconds >= self.break_target {
            Some(Trigger::BreakElapsed)
        } else {
            None
        }
    }

    /// 立即活动：可以在任意状态独立开始（PRD 6.3）。
    pub fn begin_break(
        &mut self,
        activity_id: Option<String>,
        activity_name: String,
        category: Option<String>,
        seconds: u64,
    ) {
        self.phase = Phase::Break;
        self.break_seconds = 0;
        self.break_target = seconds.max(10);
        self.activity_id = activity_id;
        self.activity_name = Some(activity_name);
        self.activity_category = category;
    }

    /// 活动结束：完成后重置工作周期；提前结束则记为部分完成。
    pub fn end_break(&mut self, completed: bool, keep_timing: bool) -> Trigger {
        self.break_seconds = 0;
        self.break_target = 0;
        self.activity_id = None;
        self.activity_name = None;
        self.activity_category = None;
        self.work_seconds = 0;
        self.reminder_elapsed = 0;

        if completed {
            self.cycle_index = self.cycle_index.saturating_add(1);
        }

        if keep_timing && self.plan.is_some() {
            self.phase = Phase::Working;
            Trigger::NextCycle
        } else {
            self.phase = Phase::Idle;
            Trigger::NextCycle
        }
    }

    /// 稍后提醒：延迟指定秒数后再次提醒。
    pub fn snooze(&mut self, seconds: u64) {
        self.snooze_remaining = Some(seconds);
        self.reminder_elapsed = 0;
        self.phase = Phase::Reminding;
    }

    /// 跳过本次提醒：不计入活动完成，直接进入下一个工作周期。
    pub fn skip(&mut self) -> Trigger {
        self.ignored_count = self.ignored_count.saturating_add(1);
        self.work_seconds = 0;
        self.reminder_elapsed = 0;
        self.snooze_remaining = None;
        if self.plan.is_some() {
            self.phase = Phase::Working;
        } else {
            self.phase = Phase::Idle;
        }
        Trigger::NextCycle
    }

    /// 已经活动：用户自述已完成活动，直接结束本次提醒。
    pub fn acknowledge_activity(&mut self) -> Trigger {
        self.work_seconds = 0;
        self.reminder_elapsed = 0;
        self.snooze_remaining = None;
        self.cycle_index = self.cycle_index.saturating_add(1);
        if self.plan.is_some() {
            self.phase = Phase::Working;
        } else {
            self.phase = Phase::Idle;
        }
        Trigger::NextCycle
    }

    pub fn set_dnd(&mut self, seconds: u64) {
        self.dnd_remaining = Some(seconds);
    }

    pub fn clear_dnd(&mut self) {
        self.dnd_remaining = None;
    }

    pub fn tick_dnd(&mut self, seconds: u64) {
        if let Some(remaining) = self.dnd_remaining {
            let left = remaining.saturating_sub(seconds);
            self.dnd_remaining = if left == 0 { None } else { Some(left) };
        }
    }

    /// 重复提醒：用户长时间无响应时按计划的间隔再次提醒。
    pub fn should_repeat_reminder(&self) -> bool {
        if self.phase != Phase::Reminding || self.snooze_remaining.is_some() {
            return false;
        }
        let plan = match self.plan.as_ref() {
            Some(p) => p,
            None => return false,
        };
        if plan.repeat_times == 0 || plan.repeat_interval_seconds == 0 {
            return false;
        }
        let interval = plan.repeat_interval_seconds * plan.repeat_times as u64;
        self.reminder_elapsed > 0
            && self.reminder_elapsed % plan.repeat_interval_seconds == 0
            && self.reminder_elapsed <= interval
    }

    /// 是否处于勿扰（勿扰期间不发提醒）。
    pub fn in_dnd(&self) -> bool {
        self.dnd_remaining.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine_with_plan(work: u64, brk: u64) -> Machine {
        let mut m = Machine::new();
        m.set_plan(PlanSpec {
            work_seconds: work,
            break_seconds: brk,
            completion_mode: "auto".into(),
            repeat_times: 2,
            repeat_interval_seconds: 300,
        });
        m
    }

    #[test]
    fn reaches_threshold_and_triggers_reminder() {
        let mut m = machine_with_plan(60, 300);
        m.start().unwrap();
        assert_eq!(m.phase, Phase::Working);
        for _ in 0..59 {
            assert_eq!(m.tick_work(1), None);
        }
        assert_eq!(m.tick_work(1), Some(Trigger::Reminder));
        assert_eq!(m.phase, Phase::Reminding);
        assert_eq!(m.remaining_work_seconds(), 0);
    }

    #[test]
    fn snooze_delays_reminder_instead_of_repeating() {
        let mut m = machine_with_plan(60, 300);
        m.start().unwrap();
        for _ in 0..60 {
            m.tick_work(1);
        }
        m.snooze(30);
        for _ in 0..29 {
            assert_eq!(m.tick_work(1), None);
        }
        assert_eq!(m.tick_work(1), Some(Trigger::ReminderRepeated));
    }

    #[test]
    fn break_completion_resets_work_cycle() {
        let mut m = machine_with_plan(60, 120);
        m.start().unwrap();
        for _ in 0..60 {
            m.tick_work(1);
        }
        m.begin_break(None, "起身行走".into(), None, 120);
        assert_eq!(m.phase, Phase::Break);
        for _ in 0..119 {
            assert_eq!(m.tick_work(1), None);
        }
        assert_eq!(m.tick_work(1), Some(Trigger::BreakElapsed));
        m.end_break(true, true);
        assert_eq!(m.phase, Phase::Working);
        assert_eq!(m.work_seconds, 0);
        assert_eq!(m.cycle_index, 1);
    }

    #[test]
    fn skip_does_not_count_as_activity() {
        let mut m = machine_with_plan(60, 300);
        m.start().unwrap();
        for _ in 0..60 {
            m.tick_work(1);
        }
        m.skip();
        assert_eq!(m.phase, Phase::Working);
        assert_eq!(m.work_seconds, 0);
        assert_eq!(m.cycle_index, 0);
        assert_eq!(m.ignored_count, 1);
    }

    #[test]
    fn pause_keeps_progress_and_resume_continues() {
        let mut m = machine_with_plan(60, 300);
        m.start().unwrap();
        for _ in 0..20 {
            m.tick_work(1);
        }
        m.pause().unwrap();
        assert_eq!(m.phase, Phase::Paused);
        assert_eq!(m.work_seconds, 20);
        assert_eq!(m.tick_work(5), None);
        assert_eq!(m.work_seconds, 20);
        m.resume().unwrap();
        assert_eq!(m.phase, Phase::Working);
    }

    #[test]
    fn suspend_stops_accumulating_and_restores() {
        let mut m = machine_with_plan(60, 300);
        m.start().unwrap();
        for _ in 0..20 {
            m.tick_work(1);
        }
        m.suspend().unwrap();
        assert_eq!(m.phase, Phase::Suspended);
        // 挂起期间不累加久坐时长（离开座位的时间不算久坐）
        for _ in 0..30 {
            assert_eq!(m.tick_work(1), None);
        }
        assert_eq!(m.work_seconds, 20);
        m.restore_from_suspend().unwrap();
        assert_eq!(m.phase, Phase::Working);
        assert_eq!(m.work_seconds, 20);
    }

    #[test]
    fn pause_from_suspended_switches_to_manual_pause() {
        let mut m = machine_with_plan(60, 300);
        m.start().unwrap();
        m.suspend().unwrap();
        m.pause().unwrap();
        assert_eq!(m.phase, Phase::Paused);
        assert!(m.suspended_from.is_none());
    }
}
