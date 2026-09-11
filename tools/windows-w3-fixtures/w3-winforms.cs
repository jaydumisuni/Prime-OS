using System;
using System.Drawing;
using System.Windows.Forms;

internal static class Program
{
    [STAThread]
    private static void Main()
    {
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        using (var form = new Form())
        {
            form.Text = "Prime W3 Managed GUI";
            form.ClientSize = new Size(560, 300);
            form.BackColor = Color.FromArgb(32, 36, 42);
            form.StartPosition = FormStartPosition.CenterScreen;
            var timer = new Timer { Interval = 8000 };
            timer.Tick += delegate { timer.Stop(); form.Close(); };
            timer.Start();
            Application.Run(form);
        }
    }
}
