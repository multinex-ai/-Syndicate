import unittest
from unittest.mock import patch

import memq_mcp_server as server


class MemqMcpServerTests(unittest.IsolatedAsyncioTestCase):
    def test_status_uses_fixed_project_scope(self):
        with patch.object(server, "status", return_value={"configured": True}) as call:
            self.assertEqual(server.memq_status(), {"configured": True})
            call.assert_called_once_with(server.ROOT)

    async def test_recall_is_bounded_and_has_no_scope_override(self):
        async def result(name, arguments, project):
            return {"name": name, "arguments": arguments, "project": str(project)}

        with patch.object(server, "invoke", side_effect=result):
            response = await server.memq_recall(" router decisions ", 99)
        self.assertEqual(response["name"], "query_memory")
        self.assertEqual(response["arguments"], {"query": "router decisions", "top_k": 3})

    async def test_recall_rejects_empty_query(self):
        with self.assertRaises(ValueError):
            await server.memq_recall("   ")


if __name__ == "__main__":
    unittest.main()
