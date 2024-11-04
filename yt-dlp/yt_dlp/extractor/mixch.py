from .common import InfoExtractor
from ..networking.exceptions import HTTPError
from ..utils import (
    ExtractorError,
    UserNotLive,
    int_or_none,
    str_or_none,
    url_or_none,
)
from ..utils.traversal import traverse_obj


class MixchIE(InfoExtractor):
    IE_NAME = 'mixch'
    _VALID_URL = r'https?://(?:www\.)?mixch\.tv/u/(?P<id>\d+)'

    _TESTS = [{
        'url': 'https://mixch.tv/u/16943797/live',
        'skip': 'don\'t know if this live persists',
        'info_dict': {
            'id': '16943797',
            'ext': 'mp4',
            'title': '#EntView #カリナ #セブチ 2024-05-05 06:58',
            'comment_count': int,
            'view_count': int,
            'timestamp': 1714726805,
            'uploader': 'Ent.View K-news🎶💕',
            'uploader_id': '16943797',
            'live_status': 'is_live',
            'upload_date': '20240503',
        },
    }, {
        'url': 'https://mixch.tv/u/16137876/live',
        'only_matching': True,
    }]

    def _real_extract(self, url):
        video_id = self._match_id(url)
        data = self._download_json(f'https://mixch.tv/api-web/users/{video_id}/live', video_id)
        if not traverse_obj(data, ('liveInfo', {dict})):
            raise UserNotLive(video_id=video_id)

        return {
            'id': video_id,
            'uploader_id': video_id,
            **traverse_obj(data, {
                'title': ('liveInfo', 'title', {str}),
                'comment_count': ('liveInfo', 'comments', {int_or_none}),
                'view_count': ('liveInfo', 'visitor', {int_or_none}),
                'timestamp': ('liveInfo', 'created', {int_or_none}),
                'uploader': ('broadcasterInfo', 'name', {str}),
            }),
            'formats': [{
                'format_id': 'hls',
                'url': data['liveInfo']['hls'],
                'ext': 'mp4',
                'protocol': 'm3u8',
            }],
            'is_live': True,
            '__post_extractor': self.extract_comments(video_id),
        }

    def _get_comments(self, video_id):
        yield from traverse_obj(self._download_json(
            f'https://mixch.tv/api-web/lives/{video_id}/messages', video_id,
            note='Downloading comments', errnote='Failed to download comments'), (..., {
                'author': ('name', {str}),
                'author_id': ('user_id', {str_or_none}),
                'id': ('message_id', {str}, filter),
                'text': ('body', {str}),
                'timestamp': ('created', {int}),
            }))


class MixchArchiveIE(InfoExtractor):
    IE_NAME = 'mixch:archive'
    _VALID_URL = r'https?://(?:www\.)?mixch\.tv/archive/(?P<id>\d+)'

    _TESTS = [{
        'url': 'https://mixch.tv/archive/421',
        'skip': 'paid video, no DRM. expires at Jan 23',
        'info_dict': {
            'id': '421',
            'ext': 'mp4',
            'title': '96NEKO SHOW TIME',
        },
    }, {
        'url': 'https://mixch.tv/archive/1213',
        'skip': 'paid video, no DRM. expires at Dec 31, 2023',
        'info_dict': {
            'id': '1213',
            'ext': 'mp4',
            'title': '【特別トーク番組アーカイブス】Merm4id×燐舞曲 2nd LIVE「VERSUS」',
            'release_date': '20231201',
            'thumbnail': str,
        },
    }, {
        'url': 'https://mixch.tv/archive/1214',
        'only_matching': True,
    }]

    def _real_extract(self, url):
        video_id = self._match_id(url)

        try:
            info_json = self._download_json(
                f'https://mixch.tv/api-web/archive/{video_id}', video_id)['archive']
        except ExtractorError as e:
            if isinstance(e.cause, HTTPError) and e.cause.status == 401:
                self.raise_login_required()
            raise

        return {
            'id': video_id,
            'title': traverse_obj(info_json, ('title', {str})),
            'formats': self._extract_m3u8_formats(info_json['archiveURL'], video_id),
            'thumbnail': traverse_obj(info_json, ('thumbnailURL', {url_or_none})),
        }
