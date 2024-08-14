import itertools
import urllib.parse

from .common import InfoExtractor
from .sproutvideo import VidsIoIE
from .vimeo import VimeoIE
from ..networking.exceptions import HTTPError
from ..utils import (
    KNOWN_EXTENSIONS,
    ExtractorError,
    clean_html,
    determine_ext,
    int_or_none,
    mimetype2ext,
    parse_iso8601,
    smuggle_url,
    str_or_none,
    traverse_obj,
    url_or_none,
    urljoin,
)


class PatreonBaseIE(InfoExtractor):
    USER_AGENT = 'Patreon/7.6.28 (Android; Android 11; Scale/2.10)'

    def _call_api(self, ep, item_id, query=None, headers=None, fatal=True, note=None):
        if headers is None:
            headers = {}
        if 'User-Agent' not in headers:
            headers['User-Agent'] = self.USER_AGENT
        if query:
            query.update({'json-api-version': 1.0})

        try:
            return self._download_json(
                f'https://www.patreon.com/api/{ep}',
                item_id, note=note if note else 'Downloading API JSON',
                query=query, fatal=fatal, headers=headers)
        except ExtractorError as e:
            if not isinstance(e.cause, HTTPError) or mimetype2ext(e.cause.response.headers.get('Content-Type')) != 'json':
                raise
            err_json = self._parse_json(self._webpage_read_content(e.cause.response, None, item_id), item_id, fatal=False)
            err_message = traverse_obj(err_json, ('errors', ..., 'detail'), get_all=False)
            if err_message:
                raise ExtractorError(f'Patreon said: {err_message}', expected=True)
            raise


class PatreonIE(PatreonBaseIE):
    _VALID_URL = r'https?://(?:www\.)?patreon\.com/(?:creation\?hid=|posts/(?:[\w-]+-)?)(?P<id>\d+)'
    _TESTS = [{
        'url': 'http://www.patreon.com/creation?hid=743933',
        'md5': 'e25505eec1053a6e6813b8ed369875cc',
        'info_dict': {
            'id': '743933',
            'ext': 'mp3',
            'title': 'Episode 166: David Smalley of Dogma Debate',
            'description': 'md5:34d207dd29aa90e24f1b3f58841b81c7',
            'uploader': 'Cognitive Dissonance Podcast',
            'thumbnail': 're:^https?://.*$',
            'timestamp': 1406473987,
            'upload_date': '20140727',
            'uploader_id': '87145',
            'like_count': int,
            'comment_count': int,
            'uploader_url': 'https://www.patreon.com/dissonancepod',
            'channel_id': '80642',
            'channel_url': 'https://www.patreon.com/dissonancepod',
            'channel_follower_count': int,
        },
    }, {
        'url': 'http://www.patreon.com/creation?hid=754133',
        'md5': '3eb09345bf44bf60451b8b0b81759d0a',
        'info_dict': {
            'id': '754133',
            'ext': 'mp3',
            'title': 'CD 167 Extra',
            'uploader': 'Cognitive Dissonance Podcast',
            'thumbnail': 're:^https?://.*$',
            'like_count': int,
            'comment_count': int,
            'uploader_url': 'https://www.patreon.com/dissonancepod',
        },
        'skip': 'Patron-only content',
    }, {
        'url': 'https://www.patreon.com/creation?hid=1682498',
        'info_dict': {
            'id': 'SU4fj_aEMVw',
            'ext': 'mp4',
            'title': 'I\'m on Patreon!',
            'uploader': 'TraciJHines',
            'thumbnail': 're:^https?://.*$',
            'upload_date': '20150211',
            'description': 'md5:8af6425f50bd46fbf29f3db0fc3a8364',
            'uploader_id': '@TraciHinesMusic',
            'categories': ['Entertainment'],
            'duration': 282,
            'view_count': int,
            'tags': 'count:39',
            'age_limit': 0,
            'channel': 'TraciJHines',
            'channel_url': 'https://www.youtube.com/channel/UCGLim4T2loE5rwCMdpCIPVg',
            'live_status': 'not_live',
            'like_count': int,
            'channel_id': 'UCGLim4T2loE5rwCMdpCIPVg',
            'availability': 'public',
            'channel_follower_count': int,
            'playable_in_embed': True,
            'uploader_url': 'https://www.youtube.com/@TraciHinesMusic',
            'comment_count': int,
            'channel_is_verified': True,
            'chapters': 'count:4',
        },
        'params': {
            'noplaylist': True,
            'skip_download': True,
        },
    }, {
        'url': 'https://www.patreon.com/posts/episode-166-of-743933',
        'only_matching': True,
    }, {
        'url': 'https://www.patreon.com/posts/743933',
        'only_matching': True,
    }, {
        'url': 'https://www.patreon.com/posts/kitchen-as-seen-51706779',
        'md5': '96656690071f6d64895866008484251b',
        'info_dict': {
            'id': '555089736',
            'ext': 'mp4',
            'title': 'KITCHEN AS SEEN ON DEEZ NUTS EXTENDED!',
            'uploader': 'Cold Ones',
            'thumbnail': 're:^https?://.*$',
            'upload_date': '20210526',
            'description': 'md5:557a409bd79d3898689419094934ba79',
            'uploader_id': '14936315',
        },
        'skip': 'Patron-only content',
    }, {
        # m3u8 video (https://github.com/yt-dlp/yt-dlp/issues/2277)
        'url': 'https://www.patreon.com/posts/video-sketchbook-32452882',
        'info_dict': {
            'id': '32452882',
            'ext': 'mp4',
            'comment_count': int,
            'uploader_id': '4301314',
            'like_count': int,
            'timestamp': 1576696962,
            'upload_date': '20191218',
            'thumbnail': r're:^https?://.*$',
            'uploader_url': 'https://www.patreon.com/loish',
            'description': 'md5:e2693e97ee299c8ece47ffdb67e7d9d2',
            'title': 'VIDEO // sketchbook flipthrough',
            'uploader': 'Loish ',
            'tags': ['sketchbook', 'video'],
            'channel_id': '1641751',
            'channel_url': 'https://www.patreon.com/loish',
            'channel_follower_count': int,
        },
    }, {
        # bad videos under media (if media is included). Real one is under post_file
        'url': 'https://www.patreon.com/posts/premium-access-70282931',
        'info_dict': {
            'id': '70282931',
            'ext': 'mp4',
            'title': '[Premium Access + Uncut] The Office - 2x6 The Fight - Group Reaction',
            'channel_url': 'https://www.patreon.com/thenormies',
            'channel_id': '573397',
            'uploader_id': '2929435',
            'uploader': 'The Normies',
            'description': 'md5:79c9fd8778e2cef84049a94c058a5e23',
            'comment_count': int,
            'upload_date': '20220809',
            'thumbnail': r're:^https?://.*$',
            'channel_follower_count': int,
            'like_count': int,
            'timestamp': 1660052820,
            'tags': ['The Office', 'early access', 'uncut'],
            'uploader_url': 'https://www.patreon.com/thenormies',
        },
        'skip': 'Patron-only content',
    }, {
        # dead vimeo and embed URLs, need to extract post_file
        'url': 'https://www.patreon.com/posts/hunter-x-hunter-34007913',
        'info_dict': {
            'id': '34007913',
            'ext': 'mp4',
            'title': 'Hunter x Hunter | Kurapika DESTROYS Uvogin!!!',
            'like_count': int,
            'uploader': 'YaBoyRoshi',
            'timestamp': 1581636833,
            'channel_url': 'https://www.patreon.com/yaboyroshi',
            'thumbnail': r're:^https?://.*$',
            'tags': ['Hunter x Hunter'],
            'uploader_id': '14264111',
            'comment_count': int,
            'channel_follower_count': int,
            'description': 'Kurapika is a walking cheat code!',
            'upload_date': '20200213',
            'channel_id': '2147162',
            'uploader_url': 'https://www.patreon.com/yaboyroshi',
        },
    }, {
        # NSFW vimeo embed URL
        'url': 'https://www.patreon.com/posts/4k-spiderman-4k-96414599',
        'info_dict': {
            'id': '902250943',
            'ext': 'mp4',
            'title': '❤️(4K) Spiderman Girl Yeonhwa’s Gift ❤️(4K) 스파이더맨걸 연화의 선물',
            'description': '❤️(4K) Spiderman Girl Yeonhwa’s Gift \n❤️(4K) 스파이더맨걸 연화의 선물',
            'uploader': 'Npickyeonhwa',
            'uploader_id': '90574422',
            'uploader_url': 'https://www.patreon.com/Yeonhwa726',
            'channel_id': '10237902',
            'channel_url': 'https://www.patreon.com/Yeonhwa726',
            'duration': 70,
            'timestamp': 1705150153,
            'upload_date': '20240113',
            'comment_count': int,
            'like_count': int,
            'thumbnail': r're:^https?://.+',
        },
        'params': {'skip_download': 'm3u8'},
    }, {
        # multiple attachments/embeds
        'url': 'https://www.patreon.com/posts/holy-wars-solos-100601977',
        'playlist_count': 3,
        'info_dict': {
            'id': '100601977',
            'title': '"Holy Wars" (Megadeth) Solos Transcription & Lesson/Analysis',
            'description': 'md5:d099ab976edfce6de2a65c2b169a88d3',
            'uploader': 'Bradley Hall',
            'uploader_id': '24401883',
            'uploader_url': 'https://www.patreon.com/bradleyhallguitar',
            'channel_id': '3193932',
            'channel_url': 'https://www.patreon.com/bradleyhallguitar',
            'channel_follower_count': int,
            'timestamp': 1710777855,
            'upload_date': '20240318',
            'like_count': int,
            'comment_count': int,
            'thumbnail': r're:^https?://.+',
        },
        'skip': 'Patron-only content',
    }]
    _RETURN_TYPE = 'video'

    def _real_extract(self, url):
        video_id = self._match_id(url)
        post = self._call_api(
            f'posts/{video_id}', video_id, query={
                'fields[media]': 'download_url,mimetype,size_bytes',
                'fields[post]': 'comment_count,content,embed,image,like_count,post_file,published_at,title,current_user_can_view',
                'fields[user]': 'full_name,url',
                'fields[post_tag]': 'value',
                'fields[campaign]': 'url,name,patron_count',
                'json-api-use-default-includes': 'false',
                'include': 'audio,user,user_defined_tags,campaign,attachments_media',
            })
        attributes = post['data']['attributes']
        info = traverse_obj(attributes, {
            'title': ('title', {str.strip}),
            'description': ('content', {clean_html}),
            'thumbnail': ('image', ('large_url', 'url'), {url_or_none}, any),
            'timestamp': ('published_at', {parse_iso8601}),
            'like_count': ('like_count', {int_or_none}),
            'comment_count': ('comment_count', {int_or_none}),
        })

        entries = []
        idx = 0
        for include in traverse_obj(post, ('included', lambda _, v: v['type'])):
            include_type = include['type']
            if include_type == 'media':
                media_attributes = traverse_obj(include, ('attributes', {dict})) or {}
                download_url = url_or_none(media_attributes.get('download_url'))
                ext = mimetype2ext(media_attributes.get('mimetype'))

                # if size_bytes is None, this media file is likely unavailable
                # See: https://github.com/yt-dlp/yt-dlp/issues/4608
                size_bytes = int_or_none(media_attributes.get('size_bytes'))
                if download_url and ext in KNOWN_EXTENSIONS and size_bytes is not None:
                    idx += 1
                    entries.append({
                        'id': f'{video_id}-{idx}',
                        'ext': ext,
                        'filesize': size_bytes,
                        'url': download_url,
                    })

            elif include_type == 'user':
                info.update(traverse_obj(include, {
                    'uploader': ('attributes', 'full_name', {str}),
                    'uploader_id': ('id', {str_or_none}),
                    'uploader_url': ('attributes', 'url', {url_or_none}),
                }))

            elif include_type == 'post_tag':
                if post_tag := traverse_obj(include, ('attributes', 'value', {str})):
                    info.setdefault('tags', []).append(post_tag)

            elif include_type == 'campaign':
                info.update(traverse_obj(include, {
                    'channel': ('attributes', 'title', {str}),
                    'channel_id': ('id', {str_or_none}),
                    'channel_url': ('attributes', 'url', {url_or_none}),
                    'channel_follower_count': ('attributes', 'patron_count', {int_or_none}),
                }))

        # all-lowercase 'referer' so we can smuggle it to Generic, SproutVideo, Vimeo
        headers = {'referer': 'https://patreon.com/'}

        # handle Vimeo embeds
        if traverse_obj(attributes, ('embed', 'provider')) == 'Vimeo':
            v_url = urllib.parse.unquote(self._html_search_regex(
                r'(https(?:%3A%2F%2F|://)player\.vimeo\.com.+app_id(?:=|%3D)+\d+)',
                traverse_obj(attributes, ('embed', 'html', {str})), 'vimeo url', fatal=False) or '')
            if url_or_none(v_url) and self._request_webpage(
                    v_url, video_id, 'Checking Vimeo embed URL', headers=headers,
                    fatal=False, errnote=False, expected_status=429):  # 429 is TLS fingerprint rejection
                entries.append(self.url_result(
                    VimeoIE._smuggle_referrer(v_url, 'https://patreon.com/'),
                    VimeoIE, url_transparent=True))

        embed_url = traverse_obj(attributes, ('embed', 'url', {url_or_none}))
        if embed_url and (urlh := self._request_webpage(
                embed_url, video_id, 'Checking embed URL', headers=headers,
                fatal=False, errnote=False, expected_status=403)):
            # Password-protected vids.io embeds return 403 errors w/o --video-password or session cookie
            if urlh.status != 403 or VidsIoIE.suitable(embed_url):
                entries.append(self.url_result(smuggle_url(embed_url, headers)))

        post_file = traverse_obj(attributes, ('post_file', {dict}))
        if post_file:
            name = post_file.get('name')
            ext = determine_ext(name)
            if ext in KNOWN_EXTENSIONS:
                entries.append({
                    'id': video_id,
                    'ext': ext,
                    'url': post_file['url'],
                })
            elif name == 'video' or determine_ext(post_file.get('url')) == 'm3u8':
                formats, subtitles = self._extract_m3u8_formats_and_subtitles(post_file['url'], video_id)
                entries.append({
                    'id': video_id,
                    'formats': formats,
                    'subtitles': subtitles,
                })

        can_view_post = traverse_obj(attributes, 'current_user_can_view')
        comments = None
        if can_view_post and info.get('comment_count'):
            comments = self.extract_comments(video_id)

        if not entries and can_view_post is False:
            self.raise_no_formats('You do not have access to this post', video_id=video_id, expected=True)
        elif not entries:
            self.raise_no_formats('No supported media found in this post', video_id=video_id, expected=True)
        elif len(entries) == 1:
            info.update(entries[0])
        else:
            for entry in entries:
                entry.update(info)
            return self.playlist_result(entries, video_id, **info, __post_extractor=comments)

        info['id'] = video_id
        info['__post_extractor'] = comments
        return info

    def _get_comments(self, post_id):
        cursor = None
        count = 0
        params = {
            'page[count]': 50,
            'include': 'parent.commenter.campaign,parent.post.user,parent.post.campaign.creator,parent.replies.parent,parent.replies.commenter.campaign,parent.replies.post.user,parent.replies.post.campaign.creator,commenter.campaign,post.user,post.campaign.creator,replies.parent,replies.commenter.campaign,replies.post.user,replies.post.campaign.creator,on_behalf_of_campaign',
            'fields[comment]': 'body,created,is_by_creator',
            'fields[user]': 'image_url,full_name,url',
            'filter[flair]': 'image_tiny_url,name',
            'sort': '-created',
            'json-api-version': 1.0,
            'json-api-use-default-includes': 'false',
        }

        for page in itertools.count(1):

            params.update({'page[cursor]': cursor} if cursor else {})
            response = self._call_api(
                f'posts/{post_id}/comments', post_id, query=params, note=f'Downloading comments page {page}')

            cursor = None
            for comment in traverse_obj(response, (('data', ('included', lambda _, v: v['type'] == 'comment')), ...)):
                count += 1
                comment_id = comment.get('id')
                attributes = comment.get('attributes') or {}
                if comment_id is None:
                    continue
                author_id = traverse_obj(comment, ('relationships', 'commenter', 'data', 'id'))
                author_info = traverse_obj(
                    response, ('included', lambda _, v: v['id'] == author_id and v['type'] == 'user', 'attributes'),
                    get_all=False, expected_type=dict, default={})

                yield {
                    'id': comment_id,
                    'text': attributes.get('body'),
                    'timestamp': parse_iso8601(attributes.get('created')),
                    'parent': traverse_obj(comment, ('relationships', 'parent', 'data', 'id'), default='root'),
                    'author_is_uploader': attributes.get('is_by_creator'),
                    'author_id': author_id,
                    'author': author_info.get('full_name'),
                    'author_thumbnail': author_info.get('image_url'),
                }

            if count < traverse_obj(response, ('meta', 'count')):
                cursor = traverse_obj(response, ('data', -1, 'id'))

            if cursor is None:
                break


class PatreonCampaignIE(PatreonBaseIE):

    _VALID_URL = r'https?://(?:www\.)?patreon\.com/(?!rss)(?:(?:m|api/campaigns)/(?P<campaign_id>\d+)|(?P<vanity>[-\w]+))'
    _TESTS = [{
        'url': 'https://www.patreon.com/dissonancepod/',
        'info_dict': {
            'title': 'Cognitive Dissonance Podcast',
            'channel_url': 'https://www.patreon.com/dissonancepod',
            'id': '80642',
            'description': 'md5:eb2fa8b83da7ab887adeac34da6b7af7',
            'channel_id': '80642',
            'channel': 'Cognitive Dissonance Podcast',
            'age_limit': 0,
            'channel_follower_count': int,
            'uploader_id': '87145',
            'uploader_url': 'https://www.patreon.com/dissonancepod',
            'uploader': 'Cognitive Dissonance Podcast',
            'thumbnail': r're:^https?://.*$',
        },
        'playlist_mincount': 68,
    }, {
        'url': 'https://www.patreon.com/m/4767637/posts',
        'info_dict': {
            'title': 'Not Just Bikes',
            'id': '4767637',
            'channel_id': '4767637',
            'channel_url': 'https://www.patreon.com/notjustbikes',
            'description': 'md5:9f4b70051216c4d5c58afe580ffc8d0f',
            'age_limit': 0,
            'channel': 'Not Just Bikes',
            'uploader_url': 'https://www.patreon.com/notjustbikes',
            'uploader': 'Jason',
            'uploader_id': '37306634',
            'thumbnail': r're:^https?://.*$',
        },
        'playlist_mincount': 71,
    }, {
        'url': 'https://www.patreon.com/api/campaigns/4243769/posts',
        'info_dict': {
            'title': 'Second Thought',
            'channel_follower_count': int,
            'id': '4243769',
            'channel_id': '4243769',
            'channel_url': 'https://www.patreon.com/secondthought',
            'description': 'md5:69c89a3aba43efdb76e85eb023e8de8b',
            'age_limit': 0,
            'channel': 'Second Thought',
            'uploader_url': 'https://www.patreon.com/secondthought',
            'uploader': 'JT Chapman',
            'uploader_id': '32718287',
            'thumbnail': r're:^https?://.*$',
        },
        'playlist_mincount': 201,
    }, {
        'url': 'https://www.patreon.com/dissonancepod/posts',
        'only_matching': True,
    }, {
        'url': 'https://www.patreon.com/m/5932659',
        'only_matching': True,
    }, {
        'url': 'https://www.patreon.com/api/campaigns/4243769',
        'only_matching': True,
    }]

    @classmethod
    def suitable(cls, url):
        return False if PatreonIE.suitable(url) else super().suitable(url)

    def _entries(self, campaign_id):
        cursor = None
        params = {
            'fields[post]': 'patreon_url,url',
            'filter[campaign_id]': campaign_id,
            'filter[is_draft]': 'false',
            'sort': '-published_at',
            'json-api-use-default-includes': 'false',
        }

        for page in itertools.count(1):

            params.update({'page[cursor]': cursor} if cursor else {})
            posts_json = self._call_api('posts', campaign_id, query=params, note=f'Downloading posts page {page}')

            cursor = traverse_obj(posts_json, ('meta', 'pagination', 'cursors', 'next'))
            for post_url in traverse_obj(posts_json, ('data', ..., 'attributes', 'patreon_url')):
                yield self.url_result(urljoin('https://www.patreon.com/', post_url), PatreonIE)

            if cursor is None:
                break

    def _real_extract(self, url):

        campaign_id, vanity = self._match_valid_url(url).group('campaign_id', 'vanity')
        if campaign_id is None:
            webpage = self._download_webpage(url, vanity, headers={'User-Agent': self.USER_AGENT})
            campaign_id = self._search_nextjs_data(
                webpage, vanity)['props']['pageProps']['bootstrapEnvelope']['pageBootstrap']['campaign']['data']['id']

        params = {
            'json-api-use-default-includes': 'false',
            'fields[user]': 'full_name,url',
            'fields[campaign]': 'name,summary,url,patron_count,creation_count,is_nsfw,avatar_photo_url',
            'include': 'creator',
        }

        campaign_response = self._call_api(
            f'campaigns/{campaign_id}', campaign_id,
            note='Downloading campaign info', fatal=False,
            query=params) or {}

        campaign_info = campaign_response.get('data') or {}
        channel_name = traverse_obj(campaign_info, ('attributes', 'name'))
        user_info = traverse_obj(
            campaign_response, ('included', lambda _, v: v['type'] == 'user'),
            default={}, expected_type=dict, get_all=False)

        return {
            '_type': 'playlist',
            'id': campaign_id,
            'title': channel_name,
            'entries': self._entries(campaign_id),
            'description': clean_html(traverse_obj(campaign_info, ('attributes', 'summary'))),
            'channel_url': traverse_obj(campaign_info, ('attributes', 'url')),
            'channel_follower_count': int_or_none(traverse_obj(campaign_info, ('attributes', 'patron_count'))),
            'channel_id': campaign_id,
            'channel': channel_name,
            'uploader_url': traverse_obj(user_info, ('attributes', 'url')),
            'uploader_id': str_or_none(user_info.get('id')),
            'uploader': traverse_obj(user_info, ('attributes', 'full_name')),
            'playlist_count': traverse_obj(campaign_info, ('attributes', 'creation_count')),
            'age_limit': 18 if traverse_obj(campaign_info, ('attributes', 'is_nsfw')) else 0,
            'thumbnail': url_or_none(traverse_obj(campaign_info, ('attributes', 'avatar_photo_url'))),
        }
