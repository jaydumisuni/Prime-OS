package com.thetechguy.prime.mobile;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.content.pm.ResolveInfo;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.LinearGradient;
import android.graphics.Paint;
import android.graphics.RadialGradient;
import android.graphics.Shader;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.ConnectivityManager;
import android.net.NetworkCapabilities;
import android.os.BatteryManager;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import android.text.Editable;
import android.text.TextUtils;
import android.text.TextWatcher;
import android.util.DisplayMetrics;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.GridLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;

import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Date;
import java.util.List;
import java.util.Locale;

/**
 * Native Android HOME surface for Prime Mobile. Small by design:
 * actual launcher activities + verified platform controls, no fake tiles,
 * privileged system permissions, mandatory AI model or resident polling agent.
 *
 * This is a real application, not evidence of a finished handset ROM.
 */
public final class HomeActivity extends Activity {
    private static final int CYAN = Color.rgb(133, 220, 250);
    private static final int WHITE = Color.rgb(237, 244, 255);
    private static final int MUTED = Color.rgb(174, 192, 220);
    private final Handler ticker = new Handler(Looper.getMainLooper());
    private final List<AppEntry> applications = new ArrayList<>();
    private FrameLayout canvas;
    private LinearLayout appSheet;
    private GridLayout appGrid;
    private TextView clockLabel;
    private TextView stateLabel;
    private EditText search;
    private boolean showingApps;

    private final Runnable refresh = new Runnable() {
        @Override public void run() {
            updateStatus();
            ticker.postDelayed(this, 60_000L);
        }
    };

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS);
        getWindow().setStatusBarColor(Color.rgb(7, 17, 37));
        getWindow().setNavigationBarColor(Color.rgb(7, 17, 37));
        buildHome();
    }

    @Override protected void onResume() {
        super.onResume();
        ticker.removeCallbacks(refresh);
        refresh.run();
        loadApplications();
        if (showingApps) renderApplications();
    }

    @Override protected void onPause() {
        ticker.removeCallbacks(refresh);
        super.onPause();
    }

    @Override public void onBackPressed() {
        if (showingApps) {
            showApps(false);
        } else {
            // HOME should stay available; Android manages the actual task stack.
            super.onBackPressed();
        }
    }

    private int dp(float v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }

    private TextView label(String content, int sizeSp, int color, boolean strong) {
        TextView t = new TextView(this);
        t.setText(content);
        t.setTextSize(sizeSp);
        t.setTextColor(color);
        t.setIncludeFontPadding(false);
        if (strong) t.setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL));
        return t;
    }

    private GradientDrawable glass(int radius, int fill, int stroke) {
        GradientDrawable d = new GradientDrawable();
        d.setShape(GradientDrawable.RECTANGLE);
        d.setColor(fill);
        d.setCornerRadius(dp(radius));
        d.setStroke(dp(1), stroke);
        return d;
    }

    private void buildHome() {
        canvas = new FrameLayout(this);
        canvas.setFitsSystemWindows(true);
        canvas.addView(new NightSky(this), new FrameLayout.LayoutParams(-1, -1));
        setContentView(canvas);

        LinearLayout body = new LinearLayout(this);
        body.setOrientation(LinearLayout.VERTICAL);
        body.setPadding(dp(23), dp(23), dp(23), dp(22));
        canvas.addView(body, new FrameLayout.LayoutParams(-1, -1));

        LinearLayout top = new LinearLayout(this);
        top.setGravity(Gravity.CENTER_VERTICAL);
        top.setOrientation(LinearLayout.HORIZONTAL);
        body.addView(top, new LinearLayout.LayoutParams(-1, dp(44)));

        ImageView emblem = new ImageView(this);
        emblem.setImageResource(R.drawable.ic_prime);
        top.addView(emblem, new LinearLayout.LayoutParams(dp(38), dp(38)));
        TextView prime = label("Prime", 21, WHITE, true);
        LinearLayout.LayoutParams primeParams = new LinearLayout.LayoutParams(0, -2, 1f);
        primeParams.leftMargin = dp(10);
        top.addView(prime, primeParams);

        clockLabel = label("", 15, WHITE, true);
        clockLabel.setGravity(Gravity.RIGHT | Gravity.CENTER_VERTICAL);
        top.addView(clockLabel, new LinearLayout.LayoutParams(-2, -2));

        View spacer = new View(this);
        body.addView(spacer, new LinearLayout.LayoutParams(1, 0, 1f));

        LinearLayout center = new LinearLayout(this);
        center.setOrientation(LinearLayout.VERTICAL);
        body.addView(center, new LinearLayout.LayoutParams(-1, -2));
        TextView eyebrow = label("YOUR MOBILE WORKSPACE", 11, CYAN, true);
        eyebrow.setLetterSpacing(0.24f);
        center.addView(eyebrow);

        TextView heading = label("Everything.\nOn your terms.", 39, WHITE, true);
        heading.setLineSpacing(dp(4), 1f);
        LinearLayout.LayoutParams titleParams = new LinearLayout.LayoutParams(-1, -2);
        titleParams.topMargin = dp(14);
        center.addView(heading, titleParams);

        TextView subtitle = label("Apps and controls when you need them. Nothing else running by default.", 15, MUTED, false);
        subtitle.setLineSpacing(dp(4), 1f);
        LinearLayout.LayoutParams subParams = new LinearLayout.LayoutParams(-1, -2);
        subParams.topMargin = dp(14);
        subParams.bottomMargin = dp(34);
        center.addView(subtitle, subParams);

        TextView apps = action("All apps", true, () -> showApps(true));
        center.addView(apps, new LinearLayout.LayoutParams(-1, dp(58)));
        LinearLayout secondary = new LinearLayout(this);
        secondary.setOrientation(LinearLayout.HORIZONTAL);
        LinearLayout.LayoutParams rowParams = new LinearLayout.LayoutParams(-1, dp(53));
        rowParams.topMargin = dp(10);
        center.addView(secondary, rowParams);
        TextView internet = action("Internet", false, this::openInternet);
        TextView settings = action("Settings", false, this::openSettings);
        LinearLayout.LayoutParams lp1 = new LinearLayout.LayoutParams(0, -1, 1f);
        lp1.rightMargin = dp(5);
        secondary.addView(internet, lp1);
        LinearLayout.LayoutParams lp2 = new LinearLayout.LayoutParams(0, -1, 1f);
        lp2.leftMargin = dp(5);
        secondary.addView(settings, lp2);

        stateLabel = label("Reading device status…", 12, MUTED, false);
        stateLabel.setGravity(Gravity.CENTER_HORIZONTAL);
        LinearLayout.LayoutParams statusParams = new LinearLayout.LayoutParams(-1, -2);
        statusParams.topMargin = dp(22);
        body.addView(stateLabel, statusParams);

        buildAppSheet();
    }

    private TextView action(String text, boolean primary, Runnable click) {
        TextView view = label(text, 15, primary ? Color.rgb(8, 24, 48) : WHITE, true);
        view.setGravity(Gravity.CENTER);
        view.setBackground(glass(19,
            primary ? Color.rgb(139, 226, 250) : Color.argb(70, 26, 47, 86),
            primary ? Color.rgb(172, 240, 255) : Color.argb(115, 137, 195, 249)));
        view.setClickable(true);
        view.setFocusable(true);
        view.setContentDescription(text);
        view.setOnClickListener(v -> click.run());
        return view;
    }

    private void buildAppSheet() {
        appSheet = new LinearLayout(this);
        appSheet.setOrientation(LinearLayout.VERTICAL);
        appSheet.setPadding(dp(20), dp(20), dp(20), dp(6));
        appSheet.setBackground(glass(28,
            Color.rgb(11, 24, 49),
            Color.argb(130, 100, 175, 245)));
        FrameLayout.LayoutParams sheetParams = new FrameLayout.LayoutParams(-1, -1);
        sheetParams.leftMargin = dp(9);
        sheetParams.rightMargin = dp(9);
        sheetParams.topMargin = dp(7);
        sheetParams.bottomMargin = dp(7);
        canvas.addView(appSheet, sheetParams);

        LinearLayout bar = new LinearLayout(this);
        bar.setGravity(Gravity.CENTER_VERTICAL);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        appSheet.addView(bar, new LinearLayout.LayoutParams(-1, dp(46)));

        TextView title = label("Applications", 22, WHITE, true);
        bar.addView(title, new LinearLayout.LayoutParams(0, -2, 1f));
        TextView close = action("Close", false, () -> showApps(false));
        bar.addView(close, new LinearLayout.LayoutParams(dp(90), dp(42)));

        search = new EditText(this);
        search.setTextSize(15);
        search.setSingleLine(true);
        search.setTextColor(WHITE);
        search.setHintTextColor(MUTED);
        search.setHint(R.string.search_apps);
        search.setPadding(dp(16), 0, dp(16), 0);
        search.setBackground(glass(16, Color.argb(90, 32, 50, 85),
            Color.argb(100, 139, 185, 242)));
        LinearLayout.LayoutParams searchParams = new LinearLayout.LayoutParams(-1, dp(50));
        searchParams.topMargin = dp(15);
        searchParams.bottomMargin = dp(15);
        appSheet.addView(search, searchParams);

        ScrollView scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        scroll.setVerticalScrollBarEnabled(false);
        appSheet.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1f));
        appGrid = new GridLayout(this);
        appGrid.setAlignmentMode(GridLayout.ALIGN_BOUNDS);
        appGrid.setOrientation(GridLayout.HORIZONTAL);
        appGrid.setColumnCount(getResources().getConfiguration().screenWidthDp < 380 ? 3 : 4);
        scroll.addView(appGrid, new ScrollView.LayoutParams(-1, -2));

        search.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence s, int start, int count, int after) {}
            @Override public void onTextChanged(CharSequence s, int start, int before, int count) {
                renderApplications();
            }
            @Override public void afterTextChanged(Editable editable) {}
        });
        appSheet.setVisibility(View.GONE);
    }

    private void showApps(boolean show) {
        showingApps = show;
        appSheet.setVisibility(show ? View.VISIBLE : View.GONE);
        if (show) {
            loadApplications();
            renderApplications();
            search.clearFocus();
        } else {
            search.setText("");
            ((InputMethodManager) getSystemService(INPUT_METHOD_SERVICE))
                .hideSoftInputFromWindow(search.getWindowToken(), 0);
        }
    }

    private void loadApplications() {
        PackageManager pm = getPackageManager();
        Intent query = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER);
        List<ResolveInfo> resolved = pm.queryIntentActivities(query, 0);
        applications.clear();
        for (ResolveInfo entry : resolved) {
            if (entry.activityInfo == null) continue;
            String pkg = entry.activityInfo.packageName;
            String name = entry.activityInfo.name;
            if (getPackageName().equals(pkg) || pkg == null || name == null) continue;
            CharSequence title = entry.loadLabel(pm);
            applications.add(new AppEntry(
                title == null ? pkg : title.toString(), pkg, name, entry));
        }
        Collections.sort(applications, (a, b) ->
            AppCatalog.compare(a.label, a.packageName, b.label, b.packageName));
    }

    private void renderApplications() {
        if (appGrid == null || search == null) return;
        appGrid.removeAllViews();
        final String term = search.getText() == null ? "" : search.getText().toString();
        final int columns = appGrid.getColumnCount();
        DisplayMetrics display = getResources().getDisplayMetrics();
        final int available = display.widthPixels - dp(58);
        final int tileWidth = Math.max(dp(67), (available - dp(8 * columns)) / columns);
        int matches = 0;
        for (AppEntry app : applications) {
            if (!AppCatalog.matches(app.label, app.packageName, term)) continue;
            if (++matches > 128) break;
            LinearLayout tile = new LinearLayout(this);
            tile.setOrientation(LinearLayout.VERTICAL);
            tile.setGravity(Gravity.CENTER_HORIZONTAL);
            tile.setPadding(dp(3), dp(14), dp(3), dp(10));
            tile.setBackground(glass(17, Color.argb(45, 67, 105, 175),
                Color.argb(35, 145, 203, 255)));
            ImageView image = new ImageView(this);
            try { image.setImageDrawable(app.resolve.loadIcon(getPackageManager())); }
            catch (RuntimeException ignored) { image.setImageResource(R.drawable.ic_prime); }
            tile.addView(image, new LinearLayout.LayoutParams(dp(44), dp(44)));
            TextView caption = label(app.label, 11, WHITE, false);
            caption.setGravity(Gravity.CENTER);
            caption.setMaxLines(2);
            caption.setEllipsize(TextUtils.TruncateAt.END);
            LinearLayout.LayoutParams cap = new LinearLayout.LayoutParams(-1, dp(35));
            cap.topMargin = dp(7);
            tile.addView(caption, cap);
            tile.setContentDescription("Open " + app.label);
            tile.setClickable(true);
            tile.setOnClickListener(v -> launch(app));
            GridLayout.LayoutParams tileParams = new GridLayout.LayoutParams();
            tileParams.width = tileWidth;
            tileParams.height = dp(119);
            tileParams.setMargins(dp(3), dp(3), dp(3), dp(3));
            appGrid.addView(tile, tileParams);
        }
        if (matches == 0) {
            TextView empty = label("No matching installed apps", 14, MUTED, false);
            empty.setPadding(dp(10), dp(25), dp(10), dp(10));
            appGrid.addView(empty);
        }
    }

    private void launch(AppEntry entry) {
        Intent target = new Intent(Intent.ACTION_MAIN);
        target.addCategory(Intent.CATEGORY_LAUNCHER);
        target.setClassName(entry.packageName, entry.activityName);
        target.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        try {
            startActivity(target);
            showApps(false);
        } catch (RuntimeException e) {
            Toast.makeText(this, "Unable to open " + entry.label, Toast.LENGTH_SHORT).show();
        }
    }

    private void openSettings() {
        try { startActivity(new Intent(Settings.ACTION_SETTINGS)); }
        catch (RuntimeException e) {
            Toast.makeText(this, "Settings unavailable", Toast.LENGTH_SHORT).show();
        }
    }

    private void openInternet() {
        try { startActivity(new Intent(Settings.Panel.ACTION_INTERNET_CONNECTIVITY)); }
        catch (RuntimeException e) {
            try { startActivity(new Intent(Settings.ACTION_WIFI_SETTINGS)); }
            catch (RuntimeException ignored) {
                Toast.makeText(this, "Internet controls unavailable", Toast.LENGTH_SHORT).show();
            }
        }
    }

    private void updateStatus() {
        if (clockLabel == null) return;
        clockLabel.setText(new SimpleDateFormat("HH:mm", Locale.getDefault()).format(new Date()));
        int level = -1;
        Intent battery = registerReceiver(null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
        if (battery != null) {
            int amount = battery.getIntExtra(BatteryManager.EXTRA_LEVEL, -1);
            int scale = battery.getIntExtra(BatteryManager.EXTRA_SCALE, -1);
            if (amount >= 0 && scale > 0) level = Math.round(amount * 100f / scale);
        }
        String batteryText = level >= 0 ? "Battery " + level + "%" : "Battery unavailable";
        String connection = "Offline";
        ConnectivityManager manager = (ConnectivityManager) getSystemService(Context.CONNECTIVITY_SERVICE);
        if (manager != null) {
            NetworkCapabilities capabilities = manager.getNetworkCapabilities(manager.getActiveNetwork());
            if (capabilities != null) {
                boolean validated = capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED);
                String type = capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ? "Wi-Fi"
                    : capabilities.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) ? "Cellular" : "Network";
                connection = validated ? type + " connected" : type + " (no verified internet)";
            }
        }
        stateLabel.setText(connection + "  ·  " + batteryText);
    }

    private static final class AppEntry {
        final String label, packageName, activityName;
        final ResolveInfo resolve;
        AppEntry(String label, String packageName, String activityName, ResolveInfo resolve) {
            this.label=label; this.packageName=packageName;
            this.activityName=activityName; this.resolve=resolve;
        }
    }

    private static final class NightSky extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        NightSky(Context context) { super(context); setLayerType(View.LAYER_TYPE_SOFTWARE, null); }
        @Override protected void onDraw(Canvas c) {
            float w = getWidth(), h = getHeight();
            paint.setShader(new LinearGradient(0, 0, w, h,
                new int[]{0xff071125,0xff14264a,0xff160d32,0xff080e22},
                new float[]{0f,0.42f,0.75f,1f},Shader.TileMode.CLAMP));
            c.drawRect(0,0,w,h,paint);
            paint.setShader(new RadialGradient(w*0.85f,h*0.24f,w*0.85f,
                new int[]{0x77519ecb,0x154b7dde,0x0014243b},null,Shader.TileMode.CLAMP));
            c.drawCircle(w*0.85f,h*0.24f,w*0.85f,paint);
            paint.setShader(new RadialGradient(w*0.18f,h*0.77f,w*0.7f,
                new int[]{0x554b27ad,0x003f247e},null,Shader.TileMode.CLAMP));
            c.drawCircle(w*0.18f,h*0.77f,w*0.7f,paint);
            paint.setShader(null);
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(1.3f);
            for(int i=0;i<5;i++) {
                paint.setColor(Color.argb(36-i*5,138,195,255));
                c.drawCircle(w*0.93f,h*0.29f,w*(0.31f+i*0.125f),paint);
            }
            paint.setStyle(Paint.Style.FILL);
        }
    }
}
