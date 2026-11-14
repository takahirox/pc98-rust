#include <compiler.h>
#include <cpucore.h>
#include <pccore.h>
unsigned int pc98_compare_clock(void) { return (unsigned int)(CPU_CLOCK + CPU_BASECLOCK - CPU_REMCLOCK); }
unsigned int pc98_compare_hz(void) { return pccore.realclock; }

#include <sound/sound.h>
unsigned int pc98_compare_sample_rate(void) { return soundcfg.rate; }
