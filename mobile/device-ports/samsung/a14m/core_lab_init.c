/* A14m source-reference adaptation of reviewed Prime MOB-1 real-Core guest init.
 * This is an UNQUALIFIED lab bridge, never a Samsung install image or release.
 * Source basis: Prime-OS PR #224 head 028aa1cf93f782d0fe8ec295a97c71a485661947.
 * The measured SOURCE KERNEL digest is injected by the pinned CI build.
 * NO bootc status impersonation, production admission, network or disk access.
 * This test-only init executes a compile-feature-gated, explicitly unqualified
 * LAB variant of the existing primed binary. Not a bootc-qualified Core.
 */
#ifndef PRIME_A14M_PINNED_KERNEL_SHA256
#error "Supply the source-verified historical A14m boot kernel SHA256"
#endif
#if !defined(__aarch64__)
#error "AArch64-only guest harness"
#endif

typedef long isize; typedef unsigned long usize;
enum { SYS_MKDIRAT=34, SYS_MOUNT=40, SYS_OPENAT=56, SYS_CLOSE=57,
       SYS_READ=63, SYS_WRITE=64, SYS_NANOSLEEP=101,
       SYS_SOCKET=198, SYS_CONNECT=203, SYS_CLONE=220, SYS_EXECVE=221 };
#define AT_FDCWD (-100)
#define SIGCHLD 17
#define AF_UNIX 1
#define SOCK_STREAM 1

static isize call(isize n,isize a,isize b,isize c,isize d,isize e){
 register isize x0 __asm__("x0")=a,x1 __asm__("x1")=b,x2 __asm__("x2")=c,
 x3 __asm__("x3")=d,x4 __asm__("x4")=e,x8 __asm__("x8")=n;
 __asm__ volatile("svc #0":"+r"(x0):"r"(x1),"r"(x2),"r"(x3),"r"(x4),"r"(x8):"memory");
 return x0;
}
void *memset(void *p,int v,usize n){volatile unsigned char *b=p;while(n--)*b++=(unsigned char)v;return p;}
static usize slen(const char *s){usize n=0;while(s[n]&&n<131072)n++;return n;}
static void out_bytes(const char *s,usize n){usize i=0;while(i<n){isize z=call(SYS_WRITE,1,(isize)(s+i),(isize)(n-i),0,0);if(z<=0)break;i+=(usize)z;}}
static void out(const char *s){out_bytes(s,slen(s));}
static int starts(const char *s,const char *prefix){while(*prefix){if(*s++!=*prefix++)return 0;}return 1;}
static const char *find(const char *s,usize len,const char *needle){usize n=slen(needle);if(n>len)return (const char*)0;for(usize i=0;i+n<=len;i++){usize j=0;while(j<n&&s[i+j]==needle[j])j++;if(j==n)return s+i;}return (const char*)0;}
static void sleep_short(void){struct {isize s,ns;} t={0,200000000};(void)call(SYS_NANOSLEEP,(isize)&t,0,0,0,0);}
static void make(const char *path){(void)call(SYS_MKDIRAT,AT_FDCWD,(isize)path,0755,0,0);}
static int get_response(const char *request,const char *label, int expected_status){
 struct {unsigned short family;char path[108];} addr={0};addr.family=AF_UNIX;
 const char *sock="/run/prime-mob1-lab/core.sock";
 usize pathlen=slen(sock);for(usize i=0;i<pathlen;i++)addr.path[i]=sock[i];
 isize fd=call(SYS_SOCKET,AF_UNIX,SOCK_STREAM,0,0,0);
 if(fd<0)return 0;
 if(call(SYS_CONNECT,fd,(isize)&addr,(isize)(2+pathlen+1),0,0)<0){(void)call(SYS_CLOSE,fd,0,0,0,0);return 0;}
 usize n=slen(request);usize off=0;
 while(off<n){isize w=call(SYS_WRITE,fd,(isize)(request+off),(isize)(n-off),0,0);if(w<=0)break;off+=(usize)w;}
 if(off!=n){(void)call(SYS_CLOSE,fd,0,0,0,0);return 0;}
 static char bytes[131072];usize count=0;
 while(count<sizeof(bytes)-1){isize r=call(SYS_READ,fd,(isize)(bytes+count),(isize)(sizeof(bytes)-1-count),0,0);if(r<=0)break;count+=(usize)r;}
 (void)call(SYS_CLOSE,fd,0,0,0,0);bytes[count]=0;
 if(count<16||!starts(bytes,"HTTP/1.1 "))return 0;
 int code=(bytes[9]-'0')*100+(bytes[10]-'0')*10+(bytes[11]-'0');
 if(code!=expected_status)return 0;
 const char *sep=find(bytes,count,"\r\n\r\n");if(!sep)return 0;
 const char *body=sep+4;usize bodylen=count-(usize)(body-bytes);
 if(bodylen==0||bodylen>65536)return 0;
 char status[]="_status=000\n"; status[8]=(char)('0'+code/100);status[9]=(char)('0'+(code/10)%10);status[10]=(char)('0'+code%10);
 out("PRIME_MOB1_CORE_V0|");out(label);out(status);
 if(expected_status!=200){out("PRIME_MOB1_CORE_V0|");out(label);out("_rejection_verified=true\n");return 1;}
 out("PRIME_MOB1_CORE_V0|");out(label);out("_json=");out_bytes(body,bodylen);out("\n");
 return 1;
}
static void do_init(void){
 out("PRIME_A14M_SOURCE_BRIDGE|not_a_phone_boot_proof=true\n");
 out("PRIME_MOB1_CORE_V0|stage=ENTERED\n");
 make("/proc");make("/sys");make("/run");make("/run/prime-mob1-lab");
 if(call(SYS_MOUNT,(isize)"proc",(isize)"/proc",(isize)"proc",0,0)!=0){out("PRIME_MOB1_CORE_V0|error=PROC_MOUNT\n");goto idle;}
 if(call(SYS_MOUNT,(isize)"sysfs",(isize)"/sys",(isize)"sysfs",0,0)!=0){out("PRIME_MOB1_CORE_V0|error=SYS_MOUNT\n");goto idle;}
 isize pid=call(SYS_CLONE,SIGCHLD,0,0,0,0);
 if(pid<0){out("PRIME_MOB1_CORE_V0|error=CLONE\n");goto idle;}
 if(pid==0){
  static char *argv[]={"/usr/bin/primed",(char*)0};
  static char *env[]={"PRIME_MOB1_LAB=UNQUALIFIED","PRIME_STATE_DIR=/run/prime-mob1-lab","PRIME_CORE_SOCKET=/run/prime-mob1-lab/core.sock","PRIME_SYSTEMD_RUN=/usr/sbin/systemd-run","PRIME_MOB1_LAB_INPUT_DIGEST=sha256:" PRIME_A14M_PINNED_KERNEL_SHA256,"PATH=/usr/bin:/bin",(char*)0};
  (void)call(SYS_EXECVE,(isize)"/usr/bin/primed",(isize)argv,(isize)env,0,0);
  out("PRIME_MOB1_CORE_V0|error=PRIMED_EXECVE\n");for(;;)sleep_short();
 }
 out("PRIME_MOB1_CORE_V0|core_process_spawned=true\n");
 static const char *versions="GET /v1/versions HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
 static const char *missing="GET /v1/capabilities HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
 static const char *incompatible="GET /v1/capabilities HTTP/1.1\r\nHost: localhost\r\nPrime-Interface-Accept: 9.0\r\nConnection: close\r\n\r\n";
 static const char *accepted="GET /v1/capabilities HTTP/1.1\r\nHost: localhost\r\nPrime-Interface-Accept: 1.0\r\nConnection: close\r\n\r\n";
 int versions_ok=0;
 for(int i=0;i<70;i++){if(get_response(versions,"versions",200)){versions_ok=1;break;}sleep_short();}
 if(!versions_ok){out("PRIME_MOB1_CORE_V0|error=VERSION_SOCKET_UNAVAILABLE\n");goto idle;}
 if(!get_response(missing,"missing_version",400)){out("PRIME_MOB1_CORE_V0|error=NEGOTIATION_MISSING_NOT_REJECTED\n");goto idle;}
 if(!get_response(incompatible,"incompatible_version",409)){out("PRIME_MOB1_CORE_V0|error=NEGOTIATION_WRONG_NOT_REJECTED\n");goto idle;}
 if(!get_response(accepted,"capabilities",200)){out("PRIME_MOB1_CORE_V0|error=CAPABILITY_UNAVAILABLE\n");goto idle;}
 out("PRIME_MOB1_CORE_V0|stage=TRANSPORT_ONLY_UNQUALIFIED\n");
 idle:for(;;)sleep_short();
}
__attribute__((noreturn)) void _start(void){do_init();__builtin_unreachable();}
