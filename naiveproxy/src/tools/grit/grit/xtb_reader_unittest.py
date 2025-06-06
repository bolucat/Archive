#!/usr/bin/env python3
# Copyright 2012 The Chromium Authors
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

'''Unit tests for grit.xtb_reader'''


import io
import os
import sys
if __name__ == '__main__':
  sys.path.append(os.path.join(os.path.dirname(__file__), '..'))

import unittest

from grit import constants
from grit import util
from grit import xtb_reader
from grit.node import empty


class XtbReaderUnittest(unittest.TestCase):
  def testParsing(self):
    xtb_file = io.BytesIO(b'''<?xml version="1.0" encoding="UTF-8"?>
      <!DOCTYPE translationbundle>
      <translationbundle lang="fr">
        <translation id="5282608565720904145">Bingo.</translation>
        <translation id="2955977306445326147">Bongo longo.</translation>
        <translation id="238824332917605038">Hullo</translation>
        <translation id="6629135689895381486"><ph name="PROBLEM_REPORT"/> peut <ph name="START_LINK"/>utilisation excessive de majuscules<ph name="END_LINK"/>.</translation>
        <translation id="7729135689895381486">Hello
this is another line
and another

and another after a blank line.</translation>
        <translation id="2348290348203948203">
          <branch variants="variants { grammatical_gender_variant { grammatical_gender_case: OTHER } }">Je suis alle</branch>
          <branch variants="variants { grammatical_gender_variant { grammatical_gender_case: FEMININE } }">Je suis allee</branch>
          <branch variants="variants { grammatical_gender_variant { grammatical_gender_case: MASCULINE } }">Je suis alle</branch>
          <branch variants="variants { grammatical_gender_variant { grammatical_gender_case: NEUTER } }">Aller est moi</branch>
        </translation>
        <translation id="83572938742934">
          <branch variants="variants { grammatical_gender_variant { grammatical_gender_case: OTHER } }">Je suis alle a <ph name="LOCATION" /> demain</branch>
          <branch variants="variants { grammatical_gender_variant { grammatical_gender_case: FEMININE } }">Je suis allee a <ph name="LOCATION" /> demain</branch>
          <branch variants="variants { grammatical_gender_variant { grammatical_gender_case: MASCULINE } }">Je suis alle a <ph name="LOCATION" /> demain</branch>
          <branch variants="variants { grammatical_gender_variant { grammatical_gender_case: NEUTER } }">Aller est moi a <ph name="LOCATION" /> demain</branch>
        </translation>
      </translationbundle>''')

    messages = []
    def Callback(id, structure):
      messages.append((id, structure))
    xtb_reader.Parse(xtb_file, Callback)
    self.assertTrue(len(messages[0][1][constants.DEFAULT_GENDER]) == 1)
    self.assertTrue(messages[3][1][constants.DEFAULT_GENDER]
                    [0])  # PROBLEM_REPORT placeholder
    self.assertTrue(messages[4][0] == '7729135689895381486')
    self.assertTrue(messages[4][1][constants.DEFAULT_GENDER][7][1] ==
                    'and another after a blank line.')
    self.assertEqual(messages[5][1]['FEMININE'][0][1], 'Je suis allee')
    self.assertEqual(messages[6][1]['NEUTER'][0][1], 'Aller est moi a ')
    self.assertTrue(messages[6][1]['NEUTER'][1][0])  # LOCATION placeholder
    self.assertEqual(messages[6][1]['NEUTER'][2][1], ' demain')

  def testParsingIntoMessages(self):
    root = util.ParseGrdForUnittest('''
      <messages>
        <message name="ID_MEGA">Fantastic!</message>
        <message name="ID_HELLO_USER">Hello <ph name="USERNAME">%s<ex>Joi</ex></ph></message>
      </messages>''')

    msgs, = root.GetChildrenOfType(empty.MessagesNode)
    clique_mega = msgs.children[0].GetCliques()[0]
    msg_mega = clique_mega.GetMessage()
    clique_hello_user = msgs.children[1].GetCliques()[0]
    msg_hello_user = clique_hello_user.GetMessage()

    xtb_file = io.BytesIO(b'''<?xml version="1.0" encoding="UTF-8"?>
      <!DOCTYPE translationbundle>
      <translationbundle lang="is">
        <translation id="%s">Meirihattar!</translation>
        <translation id="%s">Saelir <ph name="USERNAME"/></translation>
      </translationbundle>''' % (
        msg_mega.GetId().encode('utf-8'),
        msg_hello_user.GetId().encode('utf-8')))

    xtb_reader.Parse(xtb_file,
                     msgs.UberClique().GenerateXtbParserCallback('is'))
    self.assertEqual(
        'Meirihattar!',
        clique_mega.MessageForLanguageAndGender(
            'is', constants.DEFAULT_GENDER).GetRealContent())
    self.assertTrue(
        'Saelir %s',
        clique_hello_user.MessageForLanguageAndGender(
            'is', constants.DEFAULT_GENDER).GetRealContent())

  def testIfNodesWithUseNameForId(self):
    root = util.ParseGrdForUnittest('''
      <messages>
        <message name="ID_BINGO" use_name_for_id="true">Bingo!</message>
      </messages>''')
    msgs, = root.GetChildrenOfType(empty.MessagesNode)
    clique = msgs.children[0].GetCliques()[0]
    msg = clique.GetMessage()

    xtb_file = io.BytesIO(b'''<?xml version="1.0" encoding="UTF-8"?>
      <!DOCTYPE translationbundle>
      <translationbundle lang="is">
        <if expr="is_linux">
          <translation id="ID_BINGO">Bongo!</translation>
        </if>
        <if expr="not is_linux">
          <translation id="ID_BINGO">Congo!</translation>
        </if>
      </translationbundle>''')
    xtb_reader.Parse(xtb_file,
                     msgs.UberClique().GenerateXtbParserCallback('is'),
                     target_platform='darwin')
    self.assertEqual(
        'Congo!',
        clique.MessageForLanguageAndGender(
            'is', constants.DEFAULT_GENDER).GetRealContent())

  def testParseLargeFile(self):
    def Callback(id, structure):
      pass
    path = util.PathFromRoot('grit/testdata/generated_resources_fr.xtb')
    with open(path, 'rb') as xtb:
      xtb_reader.Parse(xtb, Callback)


if __name__ == '__main__':
  unittest.main()
