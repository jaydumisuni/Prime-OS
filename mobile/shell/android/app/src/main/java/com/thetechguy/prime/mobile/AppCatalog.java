package com.thetechguy.prime.mobile;
import java.util.Locale;
/** Dependency-free, stable filtering for actual launchable apps. */
public final class AppCatalog {
    private AppCatalog(){}
    public static boolean matches(String label,String pkg,String query){
        if(query==null||query.trim().isEmpty())return true;
        String n=query.trim().toLowerCase(Locale.ROOT);
        return (label!=null&&label.toLowerCase(Locale.ROOT).contains(n))||
               (pkg!=null&&pkg.toLowerCase(Locale.ROOT).contains(n));
    }
    public static int compare(String a,String pa,String b,String pb){
        String x=a==null?"":a.toLowerCase(Locale.ROOT);
        String y=b==null?"":b.toLowerCase(Locale.ROOT);
        int cmp=x.compareTo(y);
        return cmp!=0?cmp:(pa==null?"":pa).compareTo(pb==null?"":pb);
    }
}
