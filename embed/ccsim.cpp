// ccsim.cpp — reusable embedded-CraftOS-PC simulation core, exposed via a C ABI
// so a Rust MCP server (or any host) can drive it.
//
// This generalizes embed/gps_test.cpp: it owns the emulator init, the
// continuous task-queue pump, the per-computer position map + Euclidean
// distance provider, and spawning computers from startup scripts.
//
// First exposed entry point is cc_gps_selftest(), which reproduces the proven
// GPS scenario end to end and returns 1 on success. The declarative test engine
// (cc_run_test) builds on the same primitives.

#include <filesystem>
#include <unordered_map>
#include <functional>
#include <thread>
#include <chrono>
#include <fstream>
#include <string>
#include <cmath>
#include <cstdio>
#include <cstdint>
#include <map>
#include <atomic>
#include <mutex>

#include <SDL2/SDL.h>
#include <Computer.hpp>

namespace fs = std::filesystem;
using path_t = std::filesystem::path;

// --- emulator symbols (global namespace, no public header) -------------------
extern void setROMPath(path_t path);
extern void setBasePath(path_t path);
extern void config_init();
extern void driveInit();
extern void defaultPollEvents();
extern std::thread::id mainThreadID;
extern path_t computerDir;
extern std::unordered_map<int, path_t> customDataDirs;
extern void setDistanceProvider(const std::function<double(const Computer*, const Computer*)>& func);
Computer* startComputer(int id);

// --- globals that main.cpp defines and the rest of the emulator references ----
class Terminal;
int selectedRenderer = -1;
int returnValue = 0;
bool rawClient = false;
std::string script_file;
std::string script_args;
std::string overrideHardwareDriver;
std::unordered_map<path_t, std::string> globalPluginErrors;
std::unordered_map<unsigned, uint8_t> rawClientTerminalIDs;
std::map<uint8_t, Terminal*> rawClientTerminals;
int parseArguments(const std::vector<std::string>& argv) { return -1; }

// --- simulation state --------------------------------------------------------
namespace {
struct Vec3 { double x, y, z; };
std::map<int, Vec3> g_pos;
std::mutex g_pos_mutex;
std::atomic<bool> g_inited{false};

void ensure_init(const std::string& rom, const std::string& base) {
    if (g_inited.exchange(true)) return;
    fs::path basep = base;
    fs::remove_all(basep);
    fs::create_directories(basep);
    setROMPath(fs::path(rom));
    setBasePath(basep);
    selectedRenderer = 1;                 // headless
    computerDir = basep / "computer";
    fs::create_directories(computerDir);
    config_init();
    SDL_Init(SDL_INIT_TIMER);
    driveInit();
    setDistanceProvider([](const Computer* a, const Computer* b) -> double {
        std::lock_guard<std::mutex> lk(g_pos_mutex);
        auto i = g_pos.find(a->id), j = g_pos.find(b->id);
        if (i == g_pos.end() || j == g_pos.end()) return 0;
        double dx = i->second.x - j->second.x, dy = i->second.y - j->second.y, dz = i->second.z - j->second.z;
        return std::sqrt(dx*dx + dy*dy + dz*dz);
    });
    // Continuous task-queue pump (timers/sleep/gps schedule via queueTask).
    std::thread([]() {
        mainThreadID = std::this_thread::get_id();
        while (true) { try { defaultPollEvents(); } catch (...) {} }
    }).detach();
}

void spawn(int id, Vec3 p, const std::string& startup) {
    { std::lock_guard<std::mutex> lk(g_pos_mutex); g_pos[id] = p; }
    fs::path d = computerDir / std::to_string(id);
    fs::create_directories(d);
    std::ofstream(d / "startup.lua") << startup;
    startComputer(id);
}
} // namespace

extern "C" {

// Run the canonical GPS scenario: 4 hosts + 1 client, verify trilateration.
// Returns 1 on PASS, 0 on FAIL/timeout.
int cc_gps_selftest(const char* rom) {
    ensure_init(rom ? rom : "", "/tmp/ccsim-selftest");

    auto hostStartup = [](int x, int y, int z) {
        char buf[256];
        snprintf(buf, sizeof(buf),
            "periphemu.create('top','modem')\nshell.run('gps','host',%d,%d,%d)\n", x, y, z);
        return std::string(buf);
    };
    spawn(0, {0, 0, 0}, hostStartup(0, 0, 0));
    spawn(1, {10, 0, 0}, hostStartup(10, 0, 0));
    spawn(2, {0, 10, 0}, hostStartup(0, 10, 0));
    spawn(3, {0, 0, 10}, hostStartup(0, 0, 10));
    spawn(4, {3, 4, 5},
        "periphemu.create('top','modem')\n"
        "sleep(2)\n"
        "local x,y,z = gps.locate(8)\n"
        "local f = fs.open('/result.txt','w')\n"
        "if x then f.write(math.floor(x+0.5)..','..math.floor(y+0.5)..','..math.floor(z+0.5))\n"
        "else f.write('nil') end\n"
        "f.close()\n");

    fs::path res = computerDir / "4" / "result.txt";
    for (int i = 0; i < 200; i++) {                  // up to 20s
        if (fs::exists(res)) {
            std::ifstream in(res); std::string out; std::getline(in, out);
            if (!out.empty()) return out == "3,4,5" ? 1 : 0;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
    return 0;
}

} // extern "C"
