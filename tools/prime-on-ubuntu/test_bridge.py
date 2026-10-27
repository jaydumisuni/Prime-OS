#!/usr/bin/env python3
import importlib.util
import json
from pathlib import Path
import unittest
import uuid

s = importlib.util.spec_from_file_location("bridge", Path(__file__).with_name("bridge.py"))
b = importlib.util.module_from_spec(s)
s.loader.exec_module(b)

class BridgeTests(unittest.TestCase):
    def setUp(self):
        self.key = str(uuid.uuid4())
        self.apps = {self.key: ("Files", "thunar.desktop", "abcd")}
        self.launched = []
    def invoke(self, method, path, body=b""):
        return b.dispatch(method, path, body, self.apps, self.launched.append)
    def test_preview_not_primed(self):
        code, data = self.invoke("GET", "/v1/applications")
        self.assertEqual(code, 200)
        self.assertEqual(data["host_id"], b.HOST_SENTINEL)
        self.assertEqual(data["generation_id"], "UBUNTU_PREVIEW_NOT_PRIME")
        self.assertEqual(data["applications"][0]["compatibility"]["state"], "RECOGNIZED")
    def test_allowlist_and_rejection(self):
        body = json.dumps({"schema":"prime.shell-launch-request.v1", "application_id":self.key}).encode()
        self.assertEqual(self.invoke("POST", "/v1/shell/launch", body)[0], 202)
        self.assertEqual(self.launched, ["thunar.desktop"])
        wrong = json.dumps({"schema":"prime.shell-launch-request.v1", "application_id":str(uuid.uuid4())}).encode()
        self.assertEqual(self.invoke("POST", "/v1/shell/launch", wrong)[0], 403)
        self.assertEqual(self.launched, ["thunar.desktop"])
    def test_bad_schema(self):
        for body in (b"oops", b"{}", b'{"schema":"other"}'):
            self.assertEqual(self.invoke("POST", "/v1/shell/launch", body)[0], 400)
        self.assertFalse(self.launched)
    def test_core_mutations_disabled(self):
        self.assertEqual(self.invoke("POST", "/v1/system/power")[0], 403)
        self.assertEqual(self.invoke("GET", "/v1/storage")[0], 503)
    def test_launch_failure_is_not_success(self):
        body = json.dumps({"schema":"prime.shell-launch-request.v1", "application_id":self.key}).encode()
        def failure(_):
            raise OSError("Cannot launch")
        self.assertEqual(b.dispatch("POST", "/v1/shell/launch", body, self.apps, failure)[0], 503)


class CatalogueTests(unittest.TestCase):
    def test_enumerates_more_than_eleven_without_invented_entries(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as root:
            folder=Path(root)
            for i in range(21):
                (folder/f"app-{i:02d}.desktop").write_text(
                    f"[Desktop Entry]\nType=Application\nName=Prime Test {i}\nExec=/bin/true\n"
                )
            found=b.discover_desktops([folder])
            self.assertEqual(len(found),21)
            self.assertEqual(found["app-00.desktop"][0],"Prime Test 0")

    def test_hidden_and_symlinked_desktop_entries_are_excluded(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as root:
            folder=Path(root)
            valid=folder/"real.desktop"
            valid.write_text("[Desktop Entry]\nType=Application\nName=Visible\nExec=/bin/true\n")
            (folder/"hidden.desktop").write_text(
                "[Desktop Entry]\nType=Application\nName=Hidden\nExec=/bin/true\nNoDisplay=true\n"
            )
            (folder/"alias.desktop").symlink_to(valid)
            found=b.discover_desktops([folder])
            self.assertEqual(list(found),["real.desktop"])


class FileSearchTests(unittest.TestCase):
    def test_local_index_and_search_excludes_hidden_and_symlinks(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            docs=root/"Documents"
            docs.mkdir()
            (docs/"Plan 2026.txt").write_text("example")
            (docs/".secret").write_text("hidden")
            (docs/"alias.txt").symlink_to(docs/"Plan 2026.txt")
            nested=docs/"Invoices"
            nested.mkdir()
            (nested/"October Invoice.pdf").write_text("example")
            indexed=b.index_user_files(root)
            self.assertEqual([item["name"] for item in indexed],
                             ["Plan 2026.txt","October Invoice.pdf"])
            self.assertEqual(len(b.search_preview_files("invoice",indexed)),1)
            self.assertEqual(b.search_preview_files("secret",indexed),[])

    def test_only_indexed_files_can_be_dispatched(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            doc=root/"readme.txt"
            doc.write_text("data")
            files=[{"name":"readme.txt","path":str(doc),"kind":"file"}]
            opened=[]
            self.assertEqual(b.open_indexed_file(str(doc),files,opened.append)[0],202)
            self.assertEqual(opened,[str(doc)])
            self.assertEqual(b.open_indexed_file("/etc/passwd",files,opened.append)[0],403)
            doc.unlink()
            self.assertEqual(b.open_indexed_file(str(doc),files,opened.append)[0],404)

    def test_search_api_returns_matching_local_files_only(self):
        file={"name":"Budget 2026.xlsx","path":"/home/example/Documents/Budget 2026.xlsx","kind":"file"}
        body=json.dumps({"query":"budget"}).encode()
        status,result=b.dispatch("POST","/v1/preview/search",body,{},lambda _:None,[file])
        self.assertEqual(status,200)
        self.assertEqual(result["schema"],"prime.ubuntu.file-search.v1")
        self.assertEqual(result["results"],[file])


class PrimeSystemAppContracts(unittest.TestCase):
    def test_pending_prime_system_entries_cannot_launch(self):
        name="Store"
        desktop="prime-pending:store"
        key=str(b.uuid.uuid5(b.NAMESPACE,desktop))
        apps={key:(name,desktop,"abcd")}
        projection=b.project(apps)
        self.assertFalse(projection["applications"][0]["launch_ready"])
        launched=[]
        body=json.dumps({"schema":"prime.shell-launch-request.v1","application_id":key}).encode()
        status,_=b.dispatch("POST","/v1/shell/launch",body,apps,launched.append)
        self.assertEqual(status,403)
        self.assertEqual(launched,[])

    def test_machine_has_single_explicit_ubuntu_target(self):
        desktop="prime-system:machine"
        key=str(b.uuid.uuid5(b.NAMESPACE,desktop))
        apps={key:("Machine",desktop,"abcd")}
        launched=[]
        opened=[]
        body=json.dumps({"schema":"prime.shell-launch-request.v1","application_id":key}).encode()
        status,_=b.dispatch("POST","/v1/shell/launch",body,apps,launched.append,None,opened.append)
        self.assertEqual(status,202)
        self.assertEqual(opened,["computer:///"])
        self.assertEqual(launched,[])

    def test_desktop_icon_dispatch_only_allows_known_actions(self):
        opened=[]
        status,_=b.dispatch("POST","/v1/preview/open-system",
            json.dumps({"target":"bin"}).encode(),{},lambda _:None,None,opened.append)
        self.assertEqual(status,202)
        self.assertEqual(opened,["trash:///"])
        invalid,_=b.dispatch("POST","/v1/preview/open-system",
            json.dumps({"target":"../../etc/shadow"}).encode(),{},lambda _:None,None,opened.append)
        self.assertEqual(invalid,403)


class PreviewTerminalDispatchTests(unittest.TestCase):
    def test_ptyxis_singleton_is_isolated_into_natively_supported_standalone(self):
        actual=b.desktop_dispatch_argv("org.gnome.Ptyxis.desktop")
        self.assertIn(actual[0],("/usr/bin/ptyxis","/usr/bin/gtk-launch"))
        if actual[0] == "/usr/bin/ptyxis":
            self.assertEqual(actual[1:],["--standalone"])

    def test_arbitrary_application_does_not_gain_special_arguments(self):
        self.assertEqual(b.desktop_dispatch_argv("thunar.desktop"),
                         ["/usr/bin/gtk-launch","thunar.desktop"])


class DesktopLabelTests(unittest.TestCase):
    def test_distinct_installed_file_managers_keep_distinct_names(self):
        self.assertEqual(b.disambiguate_desktop_label("Files","org.gnome.Nautilus.desktop"),"GNOME Files")
        self.assertEqual(b.disambiguate_desktop_label("Settings","org.gnome.Settings.desktop"),"GNOME Settings")
        self.assertEqual(b.disambiguate_desktop_label("Files","thunar.desktop"),"Files")

if __name__ == "__main__":
    unittest.main()
