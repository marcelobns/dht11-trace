/// What the frame read needs from the platform and `embedded-hal` 1.0 does not define: a
/// counter to time the pulses and a way to keep the read from being interrupted.
pub trait Platform {
    /// Current value of a free-running counter, such as the CPU cycle counter.
    ///
    /// It must count up, wrap from `u32::MAX` to 0 and keep counting inside
    /// [`Platform::uninterrupted`]. A narrower or down-counting timer, such as the 24-bit
    /// SysTick, needs extending to that form first. If the counter stops, each wait gives up
    /// after a bounded number of polls and the read fails instead of hanging.
    fn ticks(&self) -> u32;

    /// Counter ticks per microsecond. One tick per microsecond is enough to tell 0 bits from
    /// 1 bits.
    fn ticks_per_us(&self) -> u32;

    /// Runs `read` without interruption and returns its result.
    ///
    /// The frame takes about 4 ms and tells bits apart by pulses of 26 µs and 70 µs, so an
    /// interrupt or a task switch of a few tens of microseconds corrupts it. Mask interrupts
    /// here, or call `read` directly where nothing can preempt it.
    ///
    /// A `critical-section` implementation is not always enough, because some only exclude
    /// other critical sections. The one in `esp-idf-hal`, for instance, is a FreeRTOS mutex and
    /// lets the Wi-Fi task preempt the read.
    fn uninterrupted<R>(&self, read: impl FnOnce() -> R) -> R;
}
