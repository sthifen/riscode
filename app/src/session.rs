//! Una ejecución del programa: el programa ensamblado, la máquina y el reloj.

use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use riscode_asm::Program;
use riscode_sim::{InputKind, Machine, Stop};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub enum RunState {
    Running,
    Paused,
    WaitingInput(InputKind),
    Finished(i32),
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Speed {
    Max,
    /// Instrucciones por segundo
    Limited(f64),
}

pub struct Session {
    pub prog: Program,
    pub machine: Machine,
    pub doc_id: u64,
    pub doc_version: u64,
    pub state: RunState,
    /// Hasta cuándo duerme el programa (syscall 32).
    sleep_until: Option<Instant>,
    /// "Hora" en la que va el programa, para que las pausas sumen bien aunque la pantalla
    /// solo se dibuje 60 veces por segundo.
    clock: Instant,
    budget: f64,
    ips_window: (Instant, u64),
    pub ips: f64,
    /// Registros antes de la última tanda (para resaltar los que cambiaron).
    pub prev_regs: [u32; 32],
}

pub fn now_epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Session {
    pub fn new(prog: Program, doc_id: u64, doc_version: u64, state: RunState) -> Self {
        let machine = Machine::new(&prog);
        let now = Instant::now();
        Session {
            prev_regs: machine.regs,
            prog,
            machine,
            doc_id,
            doc_version,
            state,
            sleep_until: None,
            clock: now,
            budget: 0.0,
            ips_window: (now, 0),
            ips: 0.0,
        }
    }

    pub fn resume(&mut self) {
        if matches!(self.state, RunState::Paused) {
            self.state = RunState::Running;
            self.clock = Instant::now();
            self.sleep_until = None;
        }
    }

    pub fn is_live(&self) -> bool {
        matches!(self.state, RunState::Running | RunState::Paused | RunState::WaitingInput(_))
    }

    fn apply_stop(&mut self, stop: Stop, now: Instant) -> bool {
        match stop {
            Stop::Budget => true,
            Stop::Sleep(ms) => {
                // Sumamos la pausa al reloj del programa, sin dejar que se atrase más de 100 ms.
                let base = self.clock.max(now - Duration::from_millis(100));
                self.clock = base + Duration::from_millis(ms as u64);
                if self.clock > now {
                    self.sleep_until = Some(self.clock);
                    false
                } else {
                    true
                }
            }
            Stop::Exit(c) => {
                self.state = RunState::Finished(c);
                false
            }
            Stop::Breakpoint => {
                self.state = RunState::Paused;
                false
            }
            Stop::NeedInput(k) => {
                self.state = RunState::WaitingInput(k);
                false
            }
            Stop::Error(e) => {
                self.state = RunState::Error(e);
                false
            }
        }
    }

    /// Ejecuta lo que toca en este cuadro. Devuelve cuánto esperar para el siguiente.
    pub fn tick(&mut self, breakpoints: &HashSet<u32>, speed: Speed, dt: f32) -> Option<Duration> {
        if self.state != RunState::Running {
            return None;
        }
        self.prev_regs = self.machine.regs;
        let now = Instant::now();
        if let Some(t) = self.sleep_until {
            if now < t {
                return Some(t - now);
            }
            self.sleep_until = None;
        }
        self.machine.now_ms = now_epoch_ms();
        let start = Instant::now();
        let mut executed = 0u64;
        match speed {
            Speed::Max => loop {
                let (n, stop) = self.machine.run(20_000, breakpoints);
                executed += n;
                if !self.apply_stop(stop, now) || start.elapsed() > Duration::from_millis(12) {
                    break;
                }
            },
            Speed::Limited(ips) => {
                self.budget = (self.budget + ips * dt as f64).min(ips.max(1.0));
                while self.budget >= 1.0 {
                    self.budget -= 1.0;
                    let (n, stop) = self.machine.run(1, breakpoints);
                    executed += n;
                    if !self.apply_stop(stop, now) {
                        break;
                    }
                }
            }
        }
        // instrucciones por segundo
        self.ips_window.1 += executed;
        let el = self.ips_window.0.elapsed().as_secs_f64();
        if el > 0.5 {
            self.ips = self.ips_window.1 as f64 / el;
            self.ips_window = (Instant::now(), 0);
        }
        match self.sleep_until {
            Some(t) if self.state == RunState::Running => Some(t.saturating_duration_since(Instant::now())),
            _ => Some(Duration::ZERO),
        }
    }

    pub fn step(&mut self) {
        self.prev_regs = self.machine.regs;
        self.machine.now_ms = now_epoch_ms();
        let stop = self.machine.step();
        self.state = RunState::Paused;
        match stop {
            Stop::Exit(c) => self.state = RunState::Finished(c),
            Stop::Error(e) => self.state = RunState::Error(e),
            Stop::NeedInput(k) => self.state = RunState::WaitingInput(k),
            _ => {}
        }
    }

    /// Línea del fuente donde está el pc.
    pub fn current_line(&self) -> Option<usize> {
        self.prog.line_of_pc(self.machine.pc)
    }
}
