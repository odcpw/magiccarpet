import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from voxel_capture_support import load_inputs, validate_receipts, validate_replay_save, wait_and_inject

INITIAL = 'MC_VOXEL_RECEIPT kind=state generation=2 tick=4 revision=0 content_hash=123 native_removed=0 visual_removed=0 pending=0 restored=1 world_epoch=2 house_slot=38 native_ground=0 visual_ground=0 native_house=0 visual_house=0'
IMPACT = 'MC_VOXEL_RECEIPT kind=impact generation=2 world_epoch=2 tick=8 sequence=1 slot=9 slot_generation=3 target=273 has_target=1 revision=1 geometry_changed=1'
PROJECTILE = 'MC_VOXEL_RECEIPT kind=projectile generation=2 world_epoch=2 tick=7 slot=9 slot_generation=3 source=1 model=0 x=0 y=0 z=0'
DONE = 'MC_VOXEL_RECEIPT kind=state generation=2 tick=9 revision=1 content_hash=456 native_removed=5 visual_removed=5 pending=0 restored=0 world_epoch=2 house_slot=38 native_ground=3 visual_ground=3 native_house=2 visual_house=2'
DEATH = 'MC_VOXEL_RECEIPT kind=death generation=2 tick=10 sequence=1 slot=273 slot_generation=4 source=273 applied=1'
EMITTED = 'MC_VOXEL_NATIVE_DEATH tick=11 slot=273 model=4 cells=120 generation=2 slot_generation=4 source=273'


class NativeEvidenceTests(unittest.TestCase):
    def test_receipts_require_real_impact_and_converged_geometry(self):
        result=validate_receipts('\n'.join([INITIAL,PROJECTILE,IMPACT,DONE]),impact=True,geometry=True,visible=True,same_projectile=True,target_slot=273,ground=True,house_slot=38)
        self.assertEqual(result['final']['native_removed'],'5')
        self.assertEqual(len(result['matched_projectile_impacts']),1)
        with self.assertRaises(ValueError):validate_receipts('\n'.join([INITIAL,IMPACT,DONE]),house_slot=39)
        with self.assertRaises(ValueError):validate_receipts('\n'.join([INITIAL,IMPACT,DONE.replace('visual_house=2','visual_house=1')]),geometry=True)
        with self.assertRaises(ValueError):validate_receipts('\n'.join([INITIAL,PROJECTILE.replace('slot_generation=3','slot_generation=4'),IMPACT,DONE]),same_projectile=True)
        with self.assertRaises(ValueError):validate_receipts('\n'.join([INITIAL,PROJECTILE,IMPACT,DONE]),target_slot=274)
        with self.assertRaises(ValueError):validate_receipts(INITIAL,impact=True)
        with self.assertRaises(ValueError):validate_receipts('\n'.join([INITIAL,IMPACT,DONE.replace('visual_removed=5','visual_removed=4')]),geometry=True)
        with self.assertRaises(ValueError):validate_receipts('\n'.join([INITIAL,IMPACT,IMPACT,DONE]),impact=True)
        with self.assertRaises(ValueError):validate_receipts('\n'.join([INITIAL,IMPACT,DONE.replace('pending=0','pending=1')]),geometry=True)

    def test_death_requires_same_session_and_actor_lifetime(self):
        log='\n'.join([INITIAL,DEATH,EMITTED,DONE])
        self.assertEqual(len(validate_receipts(log,death=True)['emitted_deaths']),1)
        with self.assertRaises(ValueError):validate_receipts(log.replace('slot_generation=4 source=273 applied=1','slot_generation=5 source=273 applied=1'),death=True)
        with self.assertRaises(ValueError):validate_receipts(log.replace('generation=2 slot_generation=4 source=273','generation=3 slot_generation=4 source=273'),death=True)

    def test_death_rejects_duplicate_lifetimes_and_sequences_but_allows_slot_reuse(self):
        with self.assertRaises(ValueError):
            validate_receipts('\n'.join([INITIAL,DEATH,DEATH,EMITTED,DONE]),death=True)
        with self.assertRaises(ValueError):
            validate_receipts('\n'.join([INITIAL,DEATH,EMITTED,EMITTED,DONE]),death=True)
        reused=DEATH.replace('sequence=1','sequence=2').replace('slot_generation=4','slot_generation=5')
        reused_emission=EMITTED.replace('slot_generation=4','slot_generation=5')
        result=validate_receipts('\n'.join([INITIAL,DEATH,EMITTED,reused,reused_emission,DONE]),death=True)
        self.assertEqual(len(result['emitted_deaths']),2)
        with self.assertRaises(ValueError):
            validate_receipts('\n'.join([INITIAL,DEATH,reused.replace('sequence=2','sequence=1'),EMITTED,DONE]),death=True)
        ignored=DEATH.replace('applied=1','applied=0')
        result=validate_receipts('\n'.join([INITIAL,ignored,ignored,DEATH,EMITTED,DONE]),death=True)
        self.assertEqual(len(result['applied_deaths']),1)

    def test_death_lifetime_uses_world_epoch_across_outer_generation_rebind(self):
        initial=INITIAL
        done=DONE.replace('generation=2','generation=3')
        result=validate_receipts('\n'.join([initial,DEATH,EMITTED,done]),death=True)
        self.assertEqual(result['generation'],3)
        self.assertEqual(result['world_epoch'],2)
        same=validate_receipts('\n'.join([initial,PROJECTILE,IMPACT.replace('generation=2','generation=3'),done]),same_projectile=True)
        self.assertEqual(len(same['matched_projectile_impacts']),1)
        with self.assertRaises(ValueError):
            validate_receipts('\n'.join([initial,DEATH.replace('generation=2','generation=3'),EMITTED,done]),death=True)

    def test_restored_geometry_does_not_count_as_new_impact(self):
        restored=INITIAL.replace('native_removed=0 visual_removed=0','native_removed=5 visual_removed=5').replace('native_ground=0 visual_ground=0 native_house=0 visual_house=0','native_ground=3 visual_ground=3 native_house=2 visual_house=2')
        with self.assertRaises(ValueError):validate_receipts('\n'.join([restored,IMPACT,DONE]),geometry=True)

    def test_reset_does_not_reuse_older_impact_or_death(self):
        log='\n'.join([INITIAL,IMPACT,DONE,DEATH,EMITTED,INITIAL])
        with self.assertRaises(ValueError):validate_receipts(log,impact=True)
        with self.assertRaises(ValueError):validate_receipts(log,death=True)

    def test_save_requires_both_file_and_exact_player_acknowledgement(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'save.json';marker='MC_GAME_RUNTIME_REPLAY_SAVED path='+str(path)
            with self.assertRaises(ValueError):validate_replay_save(marker,path)
            path.write_text('{}')
            with self.assertRaises(ValueError):validate_replay_save('saved elsewhere',path)
            validate_replay_save(marker,path)

    def test_input_waits_for_real_startup_and_uses_compositor_display(self):
        class Player:
            pid=123
            def poll(self):return None
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory);log=path/'player.log';output=path/'events.json'
            log.write_text('Selected window backend: x11\nMC_VOXEL_MODERN_READY\nMC_GAME_RUNTIME_STARTUP VERIFIED\nMC_WATER_READY\n')
            events=[{'key':'space'}]
            with patch('voxel_capture_support.inject',return_value={'pid':123,'wm_class':['Unity','Unity']}) as injection:
                wait_and_inject(Player(),log,':77',events,output)
                injection.assert_called_once_with(':77',events,123,log)
            self.assertTrue(output.exists())
            with self.assertRaises(RuntimeError):wait_and_inject(Player(),log,None,events,output)
            log.write_text('MC_FATAL corrupt resource')
            with self.assertRaises(RuntimeError):wait_and_inject(Player(),log,':77',events,output)

    def test_input_script_rejects_nonfinite_or_unknown_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'events.json'
            path.write_text('[{"key":"space","hold":0.2,"wait_for":"tick=598"},{"dx":10,"dy":-5,"wait":0.1}]')
            self.assertEqual(len(load_inputs(path)),2)
            for text in ('[{"key":"space","hold":NaN}]','[{"key":"space","unknown":0}]','[{"dx":1.5}]','[{"wait":40},{"wait":40}]','[{"wait_for":""}]','[{"wait_for":123}]'):
                path.write_text(text)
                with self.assertRaises(ValueError):load_inputs(path)


if __name__ == '__main__':
    unittest.main()
