/* Prime Mobile A14m — source-reference early PID1 probe, NO phone qualification.
 * AArch64 Linux kernel only, no libc, filesystem changes, partitions or network.
 * Build in a disposable CI runner. This is NOT a full OS or normal boot service.
 * Outputs only bounded non-secret kernel source fields and parks as PID1.
 */
#if !defined(__aarch64__)
#error "This program must target AArch64"
#endif

typedef long isize;
typedef unsigned long usize;
enum { SYS_OPENAT=56, SYS_CLOSE=57, SYS_READ=63, SYS_WRITE=64, SYS_MOUNT=40, SYS_NANOSLEEP=101 };
#define AT_FDCWD (-100)
#define O_RDONLY 0

static isize syscall5(isize n,isize a,isize b,isize c,isize d,isize e){
 register isize x0 __asm__("x0")=a,x1 __asm__("x1")=b,x2 __asm__("x2")=c;
 register isize x3 __asm__("x3")=d,x4 __asm__("x4")=e,x8 __asm__("x8")=n;
 __asm__ volatile("svc #0":"+r"(x0):"r"(x1),"r"(x2),"r"(x3),"r"(x4),"r"(x8):"memory");
 return x0;
}
static usize str_len(const char *s){usize n=0;while(s[n]&&n<4096)n++;return n;}
static void write_all(isize fd,const char *p,usize n){
 while(n){isize k=syscall5(SYS_WRITE,fd,(isize)p,(isize)n,0,0);if(k<=0)break;p+=(usize)k;n-=(usize)k;}
}
static void log_text(const char *s){write_all(1,s,str_len(s));}
static void log_kernel(const char *s){
 isize fd=syscall5(SYS_OPENAT,AT_FDCWD,(isize)"/dev/kmsg",1,0,0);
 if(fd>=0){write_all(fd,s,str_len(s));(void)syscall5(SYS_CLOSE,fd,0,0,0,0);}
 log_text(s);
}
static void bounded_model(void){
 const char *paths[]={"/proc/device-tree/model","/sys/firmware/devicetree/base/model"};
 for(usize i=0;i<2;i++){
  isize fd=syscall5(SYS_OPENAT,AT_FDCWD,(isize)paths[i],O_RDONLY,0,0);
  if(fd<0)continue;
  char model[129];isize n=syscall5(SYS_READ,fd,(isize)model,128,0,0);
  (void)syscall5(SYS_CLOSE,fd,0,0,0,0);
  if(n<=0)continue;
  log_kernel("PRIME_A14M_FIRST_LIGHT|device_tree_model=");
  for(isize x=0;x<n;x++){char c=model[x];if(!c)break;if(c<32||c>126)c='?';write_all(1,&c,1);}
  log_kernel("\n");return;
 }
 log_kernel("PRIME_A14M_FIRST_LIGHT|device_tree_model=UNAVAILABLE\n");
}
void _start(void){
 log_kernel("PRIME_A14M_FIRST_LIGHT|stage=ENTERED_UNQUALIFIED\n");
 isize p=syscall5(SYS_MOUNT,(isize)"proc",(isize)"/proc",(isize)"proc",0,0);
 isize s=syscall5(SYS_MOUNT,(isize)"sysfs",(isize)"/sys",(isize)"sysfs",0,0);
 log_kernel(p==0?"PRIME_A14M_FIRST_LIGHT|proc=AVAILABLE\n":"PRIME_A14M_FIRST_LIGHT|proc=UNAVAILABLE\n");
 log_kernel(s==0?"PRIME_A14M_FIRST_LIGHT|sys=AVAILABLE\n":"PRIME_A14M_FIRST_LIGHT|sys=UNAVAILABLE\n");
 if(p==0&&s==0)bounded_model();
 log_kernel("PRIME_A14M_FIRST_LIGHT|stage=EVIDENCE_ONLY_NO_PRIME_CORE_NO_PHONE_QUALIFICATION\n");
 for(;;){struct {isize sec;isize nsec;} pause={1,0};(void)syscall5(SYS_NANOSLEEP,(isize)&pause,0,0,0,0);}
}
