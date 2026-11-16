package com.thetechguy.prime.mobile;
import org.junit.Test;
import static org.junit.Assert.*;
public final class AppCatalogTest {
    @Test public void blankSearchReturnsActualApps(){
        assertTrue(AppCatalog.matches("Settings","com.android.settings"," "));
        assertTrue(AppCatalog.matches("Settings","com.android.settings",null));
    }
    @Test public void caseInsensitiveLabelAndPackageSearch(){
        assertTrue(AppCatalog.matches("Prime Files","com.ttg.files"," FILE "));
        assertTrue(AppCatalog.matches("Files","com.example.manager","EXAMPLE"));
        assertFalse(AppCatalog.matches("Files","com.ttg.files","camera"));
    }
    @Test public void nullHandling(){
        assertFalse(AppCatalog.matches(null,null,"settings"));
        assertTrue(AppCatalog.matches(null,"com.android.settings"," SETTINGS "));
    }
    @Test public void stableOrder(){
        assertTrue(AppCatalog.compare("Camera","org.b","Settings","org.a")<0);
        assertTrue(AppCatalog.compare("Camera","org.a","Camera","org.b")<0);
        assertEquals(0,AppCatalog.compare("same","a","SAME","a"));
    }
}
