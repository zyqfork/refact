import os

from refact_webgui.webgui.selfhost_model_assigner import ModelAssigner
from self_hosting_machinery.scripts import enum_devices
from self_hosting_machinery.scripts import hf_hub_available
from refact_utils.scripts import env


def assign_gpus_if_first_run_detected(model_assigner: ModelAssigner):
    if not os.path.exists(env.CONFIG_ENUM_DEVICES):
        enum_devices.enum_gpus()
    if not os.path.exists(env.CONFIG_INFERENCE):
        model_assigner.first_run()


def convert_old_configs():
    # longthink.cfg and openai_api_worker.cfg are deprecated watchdog configs
    old_longthink = os.path.join(env.DIR_WATCHDOG_D, "longthink.cfg")
    if os.path.exists(old_longthink):
        os.unlink(old_longthink)
    openai_watchdog_cfg_fn = os.path.join(env.DIR_WATCHDOG_D, "openai_api_worker.cfg")
    if os.path.exists(openai_watchdog_cfg_fn):
        os.unlink(openai_watchdog_cfg_fn)


if __name__ == '__main__':
    convert_old_configs()
    hf_hub_available.set_hf_hub_offline_flag()  # should be before ModelAssigner
    model_assigner = ModelAssigner()
    assign_gpus_if_first_run_detected(model_assigner)
    model_assigner.models_to_watchdog_configs()   # removes deprecated models
