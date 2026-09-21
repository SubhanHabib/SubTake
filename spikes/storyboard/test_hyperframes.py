import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import hyperframes_backend as hf

class HyperFramesContracts(unittest.TestCase):
    def test_export_rejects_modified_source_before_launching_renderer(self):
        with tempfile.TemporaryDirectory() as folder:
            work=Path(folder);project=work/'composition';project.mkdir()
            (project/'index.html').write_text('reviewed')
            digest=hf.digest(project)
            (project/'index.html').write_text('unreviewed edit')
            with patch.object(hf,'command') as command:
                with self.assertRaisesRegex(ValueError,'Composition changed'):
                    hf.export(work,{'project':'composition'},digest)
                command.assert_not_called()
            self.assertFalse((work/'exports').exists())

    def test_media_changes_invalidate_export_approval(self):
        with tempfile.TemporaryDirectory() as folder:
            project=Path(folder);(project/'assets').mkdir()
            media=project/'assets/clip.mp4';media.write_bytes(b'first clip')
            before=hf.digest(project);media.write_bytes(b'other clip')
            self.assertNotEqual(before,hf.digest(project))

    def test_preview_rejects_remote_url(self):
        with patch.object(hf,'command',return_value='{"url":"https://example.org"}'):
            with self.assertRaisesRegex(ValueError,'Unexpected'):
                hf.preview(Path('/tmp/composition'))

if __name__=='__main__': unittest.main()
