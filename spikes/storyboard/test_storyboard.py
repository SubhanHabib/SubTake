import copy
import tempfile
import unittest
from pathlib import Path
import storyboard as sb

class Contracts(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.work=Path(self.temp.name)
        self.assets=[dict(id='a',kind='video',duration=8)]
        self.plan=dict(title='Feature demo',message='Show the actual change',scenes=[dict(id='intro',title='New feature',purpose='Demonstrate it',asset_id='a',start=1,duration=3,caption='Try it')])
        sb.write(self.work/'storyboard.json',dict(revision=0,assets=self.assets,plan=self.plan,approved_revision=None,feedback=[],job=None))
    def tearDown(self): self.temp.cleanup()
    def test_unknown_asset_and_out_of_bounds_media_rejected(self):
        for patch in [dict(asset_id='unknown'),dict(start=7),dict(duration=float('nan')),dict(zoom=dict(depth=2,cx=2,cy=.5))]:
            plan=copy.deepcopy(self.plan);plan['scenes'][0].update(patch)
            with self.assertRaises(ValueError):sb.validate(plan,self.assets)
    def test_approval_bound_to_exact_revision_and_conflicts_preserve_edits(self):
        sb.review(self.work,0,'approve');sb.update_plan(self.work,self.plan,0)
        self.assertIsNone(sb.state(self.work)['approved_revision'])
        with self.assertRaises(ValueError):sb.update_plan(self.work,self.plan,0)
        with self.assertRaises(ValueError):sb.review(self.work,0,'approve')
        self.assertEqual(sb.state(self.work)['revision'],1)
    def test_feedback_invalidates_approval_without_changing_plan(self):
        sb.review(self.work,0,'approve');sb.review(self.work,0,'feedback','Use a more specific opening')
        self.assertIsNone(sb.state(self.work)['approved_revision'])
        self.assertEqual(sb.state(self.work)['plan'],self.plan)
        self.assertEqual(len(sb.state(self.work)['feedback']),1)
    def test_unapproved_build_cannot_start_or_touch_outputs(self):
        with self.assertRaises(ValueError):sb.build(self.work)
        self.assertFalse((self.work/'builds').exists())
    def test_reordering_keeps_source_and_scene_identity(self):
        plan=copy.deepcopy(self.plan);plan['scenes'].append(dict(id='last',title='Result',purpose='Show the outcome',asset_id='a',start=5,duration=2))
        plan['scenes'].reverse();result=sb.update_plan(self.work,plan,0)
        self.assertEqual([s['id'] for s in result['plan']['scenes']],['last','intro'])
        self.assertEqual([s['start'] for s in result['plan']['scenes']],[5,1])
    def test_intake_refuses_existing_workspace_before_touching_media(self):
        before=(self.work/'storyboard.json').read_bytes()
        with self.assertRaises(ValueError):sb.init(self.work,self.work/'brief.txt',[])
        self.assertEqual((self.work/'storyboard.json').read_bytes(),before)

if __name__=='__main__':unittest.main()
