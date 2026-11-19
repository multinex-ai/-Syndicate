import asyncio
import copy
import tempfile
import unittest
import os
import time
from pathlib import Path
from unittest.mock import patch
import memq_client as memq


class MemorySkillTests(unittest.TestCase):
    def test_tf_path_and_explicit_target_blocked(self):
        for project,task in [('$HOME',''),('$HOME',''),(str(memq.ROOT),'Review Thermo Fisher changes'),(str(memq.ROOT),'Fix Galileo agent')]:
            with self.assertRaises(PermissionError):memq.check_scope(project,task)

    def test_linked_worktree_ownership_blocks_memory(self):
        with tempfile.TemporaryDirectory() as directory:
            owned=type('Result',(),{'returncode':0,'stdout':'$HOME'})()
            with patch.object(memq.subprocess,'run',return_value=owned):
                with self.assertRaises(PermissionError):memq.check_scope(directory)

    def test_identity_mismatch_and_missing_mapping_block(self):
        cfg={'expected_account':'maintainer@example.com','expected_namespace_organization':'multinex.ai'}
        binding={'subject':'fixture-subject','namespace':'fixture-namespace','subject_path':'subject','email_path':'email','namespace_path':'namespace','namespace_scoped_path':'scoped','namespace_organization':'multinex.ai','namespace_mapping_evidence':'synthetic fixture only'}
        principal={'subject':'fixture-subject','email':'maintainer@example.com'}
        namespace={'namespace':'fixture-namespace','scoped':True}
        memq.validate_binding(principal,namespace,binding,cfg)
        for target,key,value in [('principal','email','someone@example.test'),('principal','subject','other'),('namespace','namespace','wrong'),('namespace','scoped',False),('binding','namespace_mapping_evidence','')]:
            data=copy.deepcopy({'principal':principal,'namespace':namespace,'binding':binding});data[target][key]=value
            with self.assertRaises(PermissionError):memq.validate_binding(data['principal'],data['namespace'],data['binding'],cfg)

    def test_missing_own_credentials_fails_without_network(self):
        async def attempt():
            async with memq.session():self.fail('Unauthenticated connection yielded')
        with tempfile.TemporaryDirectory() as directory,patch.object(memq,'PRIVATE',Path(directory)/'own-client'):
            with self.assertRaises(PermissionError):asyncio.run(attempt())

    def test_oauth_subject_mapping_requires_account_evidence(self):
        cfg={'expected_account':'maintainer@example.com','expected_namespace_organization':'multinex.ai'}
        binding={'subject':'fixture','namespace':'fixture-ns','subject_path':'subject','email_path':None,'namespace_path':'namespace','namespace_scoped_path':'scoped','namespace_organization':'multinex.ai','namespace_mapping_evidence':'fixture','verified_oauth_account_mapping':{'subject':'fixture','email':'maintainer@example.com','evidence':'independent fixture'}}
        principal={'subject':'fixture'};namespace={'namespace':'fixture-ns','scoped':True}
        memq.validate_binding(principal,namespace,binding,cfg)
        binding['verified_oauth_account_mapping']['email']='different@example.test'
        with self.assertRaises(PermissionError):memq.validate_binding(principal,namespace,binding,cfg)

    def test_nested_scope_override_rejected_before_connecting(self):
        cfg=memq.config();cfg['allowed_tools']=['query_memory']
        with patch.object(memq,'config',return_value=cfg):
            with self.assertRaises(PermissionError):asyncio.run(memq.invoke('query_memory',{'filters':{'namespace':'wrong'}},memq.ROOT))

    def test_restart_keeps_absolute_expiry_and_refresh_token(self):
        async def run():
            store=memq.OwnTokenStorage()
            await store.set_tokens(memq.OAuthToken(access_token='synthetic-access',token_type='Bearer',expires_in=3600,refresh_token='synthetic-refresh'))
            path=memq.PRIVATE/'oauth-tokens.json';os.utime(path,(time.time()-7200,)*2)
            provider=memq.PersistentOAuthProvider('https://mcp.multinex.ai/mcp/v1',memq.OAuthClientMetadata(redirect_uris=['http://127.0.0.1:1234/callback']),store)
            await provider._initialize()
            self.assertFalse(provider.context.is_token_valid())
            await store.set_tokens(memq.OAuthToken(access_token='synthetic-renewed',token_type='Bearer',expires_in=3600))
            self.assertEqual((await store.get_tokens()).refresh_token,'synthetic-refresh')
        with tempfile.TemporaryDirectory() as directory,patch.object(memq,'PRIVATE',Path(directory)):
            asyncio.run(run())


if __name__=='__main__':unittest.main()
