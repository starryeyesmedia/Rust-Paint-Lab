import unittest

from new_document_layout import stable_new_document_geometry


def frame(name_y, card_y, create_y):
    return [
        f"UI_TEST_NEW_DOCUMENT_NAME_RECT: 473.1 {name_y:.1f} 160.8 25.2 1.0",
        f"UI_TEST_FORMAT_CARD_RECT|Portrait|200.0 {card_y:.1f} 190.0 132.0 1.0",
        f"UI_TEST_NEW_DOCUMENT_CREATE_RECT: 900.0 {create_y:.1f} 120.0 40.0 1.0",
    ]


class NewDocumentLayoutTests(unittest.TestCase):
    def test_uses_two_fresh_stable_frames_and_ignores_previous_layout(self):
        lines = frame(247, 167, 700)
        lines += frame(120, 280, 700)
        lines += frame(120, 280, 700)

        result = stable_new_document_geometry(lines, baseline_name_count=1)

        self.assertEqual(result[0][1], 120.0)
        self.assertEqual(result[1][1], 280.0)
        self.assertEqual(result[2][1], 700.0)
        self.assertLess(result[0][1], result[1][1])

    def test_does_not_pair_geometry_across_render_frames(self):
        lines = [
            "UI_TEST_NEW_DOCUMENT_NAME_RECT: 473.1 247.0 160.8 25.2 1.0",
            "UI_TEST_NEW_DOCUMENT_NAME_RECT: 473.1 120.0 160.8 25.2 1.0",
            "UI_TEST_FORMAT_CARD_RECT|Portrait|200.0 280.0 190.0 132.0 1.0",
            "UI_TEST_NEW_DOCUMENT_CREATE_RECT: 900.0 700.0 120.0 40.0 1.0",
        ]

        self.assertIsNone(stable_new_document_geometry(lines, baseline_name_count=0))

    def test_waits_until_the_latest_two_complete_frames_agree(self):
        lines = frame(120, 280, 700)
        lines += frame(121, 281, 701)
        lines += frame(120, 280, 700)

        self.assertIsNone(stable_new_document_geometry(lines, baseline_name_count=0))

        lines += frame(120, 280, 700)
        self.assertIsNotNone(stable_new_document_geometry(lines, baseline_name_count=0))

    def test_does_not_return_while_the_current_frame_is_still_being_logged(self):
        lines = frame(120, 280, 700) + frame(120, 280, 700)
        lines += ["UI_TEST_NEW_DOCUMENT_NAME_RECT: 473.1 121.0 160.8 25.2 1.0"]

        self.assertIsNone(stable_new_document_geometry(lines, baseline_name_count=0))

    def test_rejects_malformed_bounds(self):
        lines = [
            "UI_TEST_NEW_DOCUMENT_NAME_RECT: broken",
            "UI_TEST_FORMAT_CARD_RECT|Portrait|200.0 280.0 190.0 132.0 1.0",
            "UI_TEST_NEW_DOCUMENT_CREATE_RECT: 900.0 700.0 120.0 40.0 1.0",
        ] * 2

        self.assertIsNone(stable_new_document_geometry(lines, baseline_name_count=0))


if __name__ == "__main__":
    unittest.main()
