/* Standalone browser comparison adapter; no ROM or game bytes included. */
#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <sys/stat.h>
#include "libretro.h"
static char system_dir[1024],run_dir[1024];
static struct {char key[128],value[256];} vars[160];static int nvars;
static unsigned char keys[512],*pixels;static size_t pixel_bytes;static unsigned width,height,format;
static int mouse_x,mouse_y,mouse_left;static uint64_t samples,video_frames,nonzero;static long double energy;static int diagnostic;
static void (*run)(void);static unsigned (*guest_clock)(void),(*guest_hz)(void),(*source_rate)(void);static uint32_t previous_clock;static uint64_t clocks;
static const char *override(const char*k){
 const char *pairs[][2]={{"np2kai_model","PC-9801VX"},{"np2kai_cpu_feature","Intel i486DX"},{"np2kai_clk_base","1.9968 MHz"},{"np2kai_clk_mult","33"},{"np2kai_ExMemory","3"},{"np2kai_FastMC","OFF"},{"np2kai_uselasthddmount","OFF"},{"np2kai_inputmouse","ON"},{"np2kai_keyboard","Us"},{"np2kai_SNDboard","PC9801-86"},{"np2kai_CLGD_en","OFF"},{"np2kai_vf1","OFF"}};
 for(unsigned i=0;i<sizeof(pairs)/sizeof(*pairs);i++)if(!strcmp(k,pairs[i][0]))return pairs[i][1];return NULL;
}
static bool environment(unsigned cmd,void*data){
 switch(cmd){
 case RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY:case RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY:case RETRO_ENVIRONMENT_GET_CORE_ASSETS_DIRECTORY:*(const char**)data=system_dir;return true;
 case RETRO_ENVIRONMENT_GET_CAN_DUPE:*(bool*)data=true;return true;
 case RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION:*(unsigned*)data=0;return true;
 case RETRO_ENVIRONMENT_SET_VARIABLES:{const struct retro_variable*v=data;for(;v->key&&nvars<160;v++){
  snprintf(vars[nvars].key,128,"%s",v->key);const char*p=strchr(v->value,';');if(!p)continue;p++;while(*p==' ')p++;
  size_t n=strcspn(p,"|");if(n>255)n=255;memcpy(vars[nvars].value,p,n);vars[nvars].value[n]=0;nvars++;}return true;}
 case RETRO_ENVIRONMENT_GET_VARIABLE:{struct retro_variable*v=data;v->value=override(v->key);if(v->value)return true;for(int i=0;i<nvars;i++)if(!strcmp(v->key,vars[i].key)){v->value=vars[i].value;return true;}return false;}
 case RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE:*(bool*)data=false;return true;
 case RETRO_ENVIRONMENT_SET_PIXEL_FORMAT:format=*(unsigned*)data;return format==RETRO_PIXEL_FORMAT_RGB565||format==RETRO_PIXEL_FORMAT_XRGB8888;
 case RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME:case RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS:case RETRO_ENVIRONMENT_SET_CONTROLLER_INFO:case RETRO_ENVIRONMENT_SET_SERIALIZATION_QUIRKS:return true;
 default:return false;}
}
static void poll(void){}
static int16_t input(unsigned port,unsigned device,unsigned index,unsigned id){(void)port;(void)index;
 if(device==RETRO_DEVICE_KEYBOARD)return id<512?keys[id]:0;
 if(device==RETRO_DEVICE_MOUSE){if(id==RETRO_DEVICE_ID_MOUSE_X){int n=mouse_x;mouse_x=0;return n;}if(id==RETRO_DEVICE_ID_MOUSE_Y){int n=mouse_y;mouse_y=0;return n;}if(id==RETRO_DEVICE_ID_MOUSE_LEFT)return mouse_left;}
 return 0;
}
static void video(const void*data,unsigned w,unsigned h,size_t pitch){if(!data)return;size_t size=pitch*h;if(size!=pixel_bytes){pixels=realloc(pixels,size);pixel_bytes=size;}memcpy(pixels,data,size);width=w;height=h;video_frames++;}
static size_t audio(const int16_t*data,size_t frames){if(diagnostic)for(size_t i=0;i<frames*2;i++){nonzero+=data[i]!=0;energy+=(long double)data[i]*data[i];}samples+=frames;return frames;}
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
static void frame(void){run();uint32_t n=guest_clock();clocks+=(uint32_t)(n-previous_clock);previous_clock=n;}
static void advance(unsigned ms){uint64_t target=clocks+(uint64_t)guest_hz()*ms/1000;unsigned cap=0;while(clocks<target){frame();if(++cap>ms+10000){fprintf(stderr,"guest clock stalled\n");exit(3);}}}
static void key(unsigned k){keys[k]=1;advance(55);keys[k]=0;advance(20);}
static void command(const char*s){for(;*s;s++)key((unsigned char)*s);key(RETROK_RETURN);advance(1000);}
static void mouse(int x,int y,int buttons,unsigned ms){mouse_x=x;mouse_y=y;mouse_left=buttons;advance(ms);}
static uint32_t anchor(void){uint32_t h=2166136261;for(unsigned y=326;y<348;y++)for(unsigned x=118;x<338;x++){uint32_t bit;if(format==RETRO_PIXEL_FORMAT_RGB565){uint16_t p=((uint16_t*)pixels)[y*width+x];bit=(p>>11)>=25&&((p>>5)&63)>=50&&(p&31)>=25;}else{uint32_t p=((uint32_t*)pixels)[y*width+x];bit=((p>>16)&255)>200&&((p>>8)&255)>200&&(p&255)>200;}h=(h^bit)*16777619;}return h;}
static void snapshot(const char*name){char path[1200];snprintf(path,sizeof path,"%s/%s.ppm",run_dir,name);FILE*f=fopen(path,"wb");if(!f)exit(4);fprintf(f,"P6\n%u %u\n255\n",width,height);
 for(unsigned y=0;y<height;y++)for(unsigned x=0;x<width;x++){unsigned char rgb[3];if(format==RETRO_PIXEL_FORMAT_RGB565){uint16_t c=((uint16_t*)pixels)[y*width+x];rgb[0]=((c>>11)&31)*255/31;rgb[1]=((c>>5)&63)*255/63;rgb[2]=(c&31)*255/31;}else{uint32_t c=((uint32_t*)pixels)[y*width+x];rgb[0]=c>>16;rgb[1]=c>>8;rgb[2]=c;}fwrite(rgb,1,3,f);}fclose(f);
}
extern unsigned pc98_compare_clock(void),pc98_compare_hz(void),pc98_compare_sample_rate(void);
int main(int argc,char**argv){if(argc<5)return 1;diagnostic=argc>5&&!strcmp(argv[5],"diagnostic");snprintf(run_dir,sizeof run_dir,"%s",argv[2]);snprintf(system_dir,sizeof system_dir,"%s/system",run_dir);mkdir(system_dir,0700);char dir[1200];snprintf(dir,sizeof dir,"%s/np2kai",system_dir);mkdir(dir,0700);
 run=retro_run;guest_clock=pc98_compare_clock;guest_hz=pc98_compare_hz;source_rate=pc98_compare_sample_rate;if(!run||!guest_clock||!guest_hz)return 2;
 retro_set_environment(environment);retro_set_video_refresh(video);retro_set_audio_sample_batch(audio);retro_set_input_poll(poll);retro_set_input_state(input);retro_init();struct retro_game_info game={argv[3],NULL,0,NULL};if(!retro_load_game(&game))return 3;run();previous_clock=guest_clock();advance(12000);snapshot("boot");
 if(!strcmp(argv[4],"boot")){fprintf(stderr,"BENCH {\"phase\":\"boot\",\"hz\":%u,\"width\":%u,\"height\":%u}\n",guest_hz(),width,height);retro_deinit();return 0;}
 command("cd nanpa2");command("nanpa2");key(RETROK_RETURN);advance(1000);snapshot("start");
 for(int i=0;i<4;i++)advance(10000);snapshot("opening");for(int i=0;i<8;i++)mouse(-100,-100,0,30);for(int i=0;i<5;i++)mouse(100,50,0,30);mouse(20,0,1,100);mouse(0,0,0,1000);advance(10000);snapshot("prologue");if(anchor()!=854969663){fprintf(stderr,"scene anchor mismatch before measurement\n");return 5;}
 uint64_t begin_clock=clocks,begin_samples=samples,begin_video=video_frames,begin_nonzero=nonzero;long double begin_energy=energy;double began=now();
 uint64_t goal=clocks+(uint64_t)guest_hz()*24;while(clocks<goal){double elapsed=(double)(clocks-begin_clock)/guest_hz();(void)elapsed;mouse_left=0;frame();}double wall=now()-began;snapshot("final");if(anchor()!=854969663){fprintf(stderr,"scene anchor mismatch after measurement\n");return 5;}
 char pcm_report[180]="";if(diagnostic)snprintf(pcm_report,sizeof pcm_report,",\"pcmNonzeroSamples\":%llu,\"pcmEnergy\":%.0Lf",(unsigned long long)(nonzero-begin_nonzero),energy-begin_energy);
 fprintf(stderr,"BENCH {\"phase\":\"prologue-bell-idle\",\"guestSeconds\":%.9f,\"wallMs\":%.3f,\"hz\":%u,\"sourceSampleRate\":%u,\"videoFrames\":%llu,\"audioFrames\":%llu,\"pcmDiagnostic\":%s%s}\n",(double)(clocks-begin_clock)/guest_hz(),wall*1000,guest_hz(),source_rate(),(unsigned long long)(video_frames-begin_video),(unsigned long long)(samples-begin_samples),diagnostic?"true":"false",pcm_report);
 retro_deinit();return 0;
}
