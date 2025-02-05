import uuid

from ic.agent import Agent
from ic.canister import Canister
from ic.client import Client
from ic.identity import Identity


class GovernanceCanister:
    _instance = None

    #
    # const CANISTER_ID = "qhsyi-dyaaa-aaaai-q3s4a-cai";
    # const ICP_HOST = "https://icp0.io";
    def __new__(cls, did_file_path: str = "app/sentinel_dashboard_backend.did",
                canister_id: str = "qhsyi-dyaaa-aaaai-q3s4a-cai",
                url: str = "https://icp0.io"):
        if cls._instance is None:
            cls._instance = super(GovernanceCanister, cls).__new__(cls)
        return cls._instance

    def __init__(self, did_file_path: str = "app/sentinel_dashboard_backend.did",
                 canister_id: str = "qhsyi-dyaaa-aaaai-q3s4a-cai",
                 url: str = "https://icp0.io"):
        # Only initialize once
        if getattr(self, "_initialized", False):
            return

        # Create identity, client and agent.
        self.identity = Identity()
        self.client = Client(url=url)
        self.agent = Agent(self.identity, self.client)

        # Read the governance candid file.
        with open(did_file_path, "r") as f:
            self.governance_did = f.read()

        # Create the canister instance.
        self.governance = Canister(agent=self.agent,
                                   canister_id=canister_id,
                                   candid=self.governance_did)

        self._initialized = True

    def add_image_hash(self, subject_id: uuid.UUID, image_hash: str):
        """Call the canister method to add an image hash."""
        return self.governance.add_image_hash(str(subject_id), image_hash)

    def get_image_hashes(self, subject_id: uuid.UUID):
        """Call the canister method to get image hashes."""
        return self.governance.get_image_hashes(str(subject_id))


governance_canister_instance = GovernanceCanister()
