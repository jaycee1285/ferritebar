use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub bar: BarConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub modules: ModuleLayout,
}

#[derive(Debug, Deserialize)]
pub struct BarConfig {
    #[serde(default = "default_position")]
    pub position: Position,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default)]
    pub margin: MarginConfig,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            position: Position::Top,
            height: 32,
            margin: MarginConfig::default(),
        }
    }
}

fn default_position() -> Position {
    Position::Top
}

fn default_height() -> u32 {
    32
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Top,
    Bottom,
}

#[derive(Debug, Deserialize, Default)]
pub struct MarginConfig {
    #[serde(default)]
    pub top: i32,
    #[serde(default)]
    pub bottom: i32,
    #[serde(default)]
    pub left: i32,
    #[serde(default)]
    pub right: i32,
}

#[derive(Debug, Deserialize, Default)]
pub struct ThemeConfig {
    pub icon_theme: Option<String>,
    #[serde(default = "default_font")]
    pub font: String,
    pub success_color: Option<String>,
    pub warning_color: Option<String>,
    pub error_color: Option<String>,
}

fn default_font() -> String {
    "Spline Sans Mono".to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct ModuleLayout {
    #[serde(default)]
    pub left: Vec<ModuleConfig>,
    #[serde(default)]
    pub center: Vec<ModuleConfig>,
    #[serde(default)]
    pub right: Vec<ModuleConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ModuleConfig {
    #[serde(rename = "clock")]
    Clock(ClockConfig),
    #[serde(rename = "battery")]
    Battery(BatteryConfig),
    #[serde(rename = "audio")]
    Audio(AudioConfig),
    #[serde(rename = "network")]
    Network(NetworkConfig),
    #[serde(rename = "memory")]
    Memory(MemoryConfig),
    #[serde(rename = "swap")]
    Swap(SwapConfig),
    #[serde(rename = "tray")]
    Tray(TrayConfig),
    #[serde(rename = "taskbar")]
    Taskbar(TaskbarConfig),
    #[serde(rename = "power")]
    Power(PowerConfig),
    #[serde(rename = "script")]
    Script(ScriptConfig),
}

#[derive(Debug, Deserialize)]
pub struct ClockConfig {
    #[serde(default = "default_clock_format")]
    pub format: String,
    #[serde(default = "default_clock_tooltip_format")]
    pub tooltip_format: String,
    pub on_click: Option<String>,
}

fn default_clock_format() -> String {
    "%I:%M %p".to_string()
}

fn default_clock_tooltip_format() -> String {
    "%A, %B %d, %Y".to_string()
}

#[derive(Debug, Deserialize)]
pub struct BatteryConfig {
    #[serde(default = "default_battery_format")]
    pub format: String,
    #[serde(default = "default_battery_path")]
    pub path: String,
    #[serde(default = "default_battery_interval")]
    pub interval: u64,
    #[serde(default = "default_battery_max_charge")]
    pub max_charge: u8,
}

fn default_battery_format() -> String {
    "{icon}".to_string()
}

fn default_battery_max_charge() -> u8 {
    100
}

fn default_battery_path() -> String {
    "/sys/class/power_supply/BAT0".to_string()
}

fn default_battery_interval() -> u64 {
    30
}

#[derive(Debug, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_audio_format")]
    pub format: String,
    #[serde(default = "default_mute_command")]
    pub on_click: String,
}

fn default_audio_format() -> String {
    "{icon} {volume}%".to_string()
}

fn default_mute_command() -> String {
    "wpctl set-mute @DEFAULT_SINK@ toggle".to_string()
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_network_format")]
    pub format: String,
    #[serde(default = "default_network_interval")]
    pub interval: u64,
    pub on_click: Option<String>,
}

fn default_network_format() -> String {
    "{icon}".to_string()
}

fn default_network_interval() -> u64 {
    10
}

#[derive(Debug, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_format")]
    pub format: String,
    #[serde(default = "default_memory_interval")]
    pub interval: u64,
    #[serde(default = "default_bar_width")]
    pub bar_width: i32,
    #[serde(default = "default_bar_height")]
    pub bar_height: i32,
}

fn default_memory_format() -> String {
    "{icon}".to_string()
}

fn default_memory_interval() -> u64 {
    3
}

fn default_bar_width() -> i32 {
    40
}

fn default_bar_height() -> i32 {
    14
}

#[derive(Debug, Deserialize)]
pub struct SwapConfig {
    #[serde(default = "default_swap_format")]
    pub format: String,
    #[serde(default = "default_swap_interval")]
    pub interval: u64,
    #[serde(default = "default_bar_width")]
    pub bar_width: i32,
    #[serde(default = "default_bar_height")]
    pub bar_height: i32,
}

fn default_swap_format() -> String {
    "{icon}".to_string()
}

fn default_swap_interval() -> u64 {
    5
}

#[derive(Debug, Deserialize)]
pub struct TrayConfig {
    #[serde(default = "default_tray_icon_size")]
    pub icon_size: i32,
}

fn default_tray_icon_size() -> i32 {
    24
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskbarDisplay {
    Icon,
    Title,
    Both,
}

fn default_taskbar_display() -> TaskbarDisplay {
    TaskbarDisplay::Icon
}

#[derive(Debug, Deserialize)]
pub struct TaskbarConfig {
    #[serde(default = "default_taskbar_max_title")]
    pub max_title_length: usize,
    #[serde(default = "default_taskbar_icon_size")]
    pub icon_size: i32,
    #[serde(default = "default_taskbar_display")]
    pub display: TaskbarDisplay,
    pub on_click: Option<String>,
}

fn default_taskbar_max_title() -> usize {
    30
}

fn default_taskbar_icon_size() -> i32 {
    32
}

#[derive(Debug, Deserialize)]
pub struct PowerConfig {
    #[serde(default = "default_power_icon")]
    pub icon: String,
    #[serde(default = "default_lock_cmd")]
    pub lock_cmd: String,
    #[serde(default = "default_suspend_cmd")]
    pub suspend_cmd: String,
    #[serde(default = "default_reboot_cmd")]
    pub reboot_cmd: String,
    #[serde(default = "default_shutdown_cmd")]
    pub shutdown_cmd: String,
    pub logout_cmd: Option<String>,
}

fn default_power_icon() -> String {
    "\u{23FB}".to_string()
}

fn default_lock_cmd() -> String {
    "swaylock".to_string()
}

fn default_suspend_cmd() -> String {
    "systemctl suspend".to_string()
}

fn default_reboot_cmd() -> String {
    "systemctl reboot".to_string()
}

fn default_shutdown_cmd() -> String {
    "systemctl poweroff".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ScriptConfig {
    pub name: String,
    pub exec: String,
    #[serde(default = "default_script_interval")]
    pub interval: u64,
    pub icon: Option<String>,
    #[serde(default = "default_return_type")]
    pub return_type: String,
    pub on_click: Option<String>,
}

fn default_script_interval() -> u64 {
    30
}

fn default_return_type() -> String {
    "json".to_string()
}
