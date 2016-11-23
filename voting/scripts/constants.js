(function() {
    'use strict';

    angular.module('votingApp')
        .constant('constants', {
            elections: [
                {
                    id: 1,
                    name: 'US Presidential Election',
                    type: 'Head of State Elections',
                    ends: 'Mon, 07 Nov 2016 14:00:00 -0600',
                    description: 'An election for President of the United States occurs every four years on Election Day, held the first Tuesday after the first Monday in November. The 2016 Presidential election will be held on November 8, 2016.',
                    modulusHexadecimal: 'c6bdba96f5035e24dbef9e10c0d4ad84e84eda02484e02c526082a13628ee0f35c8b58eb75b4425de6bae37d2f3c43f7dd01b9d1b905844b730eeabb3f4df045e42f9b274f8f15efd68cc1e486af12f87e84c620204393356dbd29504934c934597531034caf23ae537a4479f6697c550c0ed9830afce839af3079727aca8cc7',
                    publicExponentHexadecimal: 10001,
                    candidates: [
                        {
                            id: 1,
                            name: 'Donald Trump',
                            code: 'DNLD_T',
                            photo: 'images/photo/trump.png',
                            link: 'https://en.wikipedia.org/wiki/Donald_Trump',
                            description: 'Donald John Trump is an American businessman, television personality, author, politician, and the Republican Party nominee for President of the United States in the 2016 election.',
                            data: [
                                {
                                    plaintext: 'DNLD_T 350380980234278027309823409809823497723',
                                    encrypted: '786cbad9a3a6ef6ab0cefd84a11f7517f010cb0f39681d13180c0c28144756e785134578f624d6de4af36bd1692507c20f97a8ca1dbdee20f9cd5110934c85f3e3c9fbb5cd8e0fc8869f0c2e6aa38ea470315cd36efe6281334122697315d881c4e71b1ebb923c9052768587a330e500280c3dc776b855f0a883cd31f97be2c4',
                                    hash: '2f906f198e99439bc8ae19e9c086e6468e21b6533d8805d92d7698a3926f695a',
                                    prefix: '2f906f19',
                                    memo: 'vice frock store',
                                    txLink: 'https://blockchain.info/tx/e19acca2b198c3fa85ee2b65839bb0a1ce9d49fd7ad43f5ae13222ab304011a3'
                                },
                                {
                                    plaintext: 'DNLD_T 734987243454082323845454923627654578711',
                                    encrypted: 'c147845df0925d2ccdc3dbbf3879d2dbe24304446497dcd7b6d1ce915b9d476a370a397b387aac000789504e565a4254350ee82a76a74d86e923a4bbd4d5a054a11df503a30ef1f998851569f9f48139c135e972b2184091e97211fa437848e0bc59bbde99832b308a3a8611da31e2772bbffe7c85076187b9580d4cadfe545a',
                                    hash: '3a8af7daf4a0ccdc958a0e126fbe753c3de9fb83945fd9922e5105256ae896c4',
                                    prefix: '3a8af7da',
                                    memo: 'potato saviour sled',
                                    txLink: 'https://blockchain.info/tx/17a40f72f52e9ea317467b0ffe622153508b7b9b937b3931eced01cda6e9a403'
                                },
                                {
                                    plaintext: 'DNLD_T 322385123093595732045482309230349340123',
                                    encrypted: '5d30d24b353ddd96c42415633024393842fb50bf06af3018d3c6a7529b9282d4ffd72c5edbe120126c106ae2fa259a70ec86fd4bcc28048880110c0d633bd41579c87b5d6ca2f7d42f9c9de4f16022c18ed556ae29b360a14d8f36f22332a66e753e5714dd6ff0bdf962057df20b61f19927532de3d151e051fcc5e748bd5e44',
                                    hash: 'c4a393a9820d5f079eee0b2a3fab1d5b116ddf75f607e25cc8f791a96d4c9460',
                                    prefix: 'c4a393a9',
                                    memo: 'recipe wheelbarrow angel',
                                    txLink: 'https://blockchain.info/tx/a17def12f90505574bca79799b3a1aca9828661626e9ea534d58d01f0fb7e653'
                                },
                            ]
                        },
                        {
                            id: 2,
                            name: 'Hillary Clinton',
                            code: 'HLLR_C',
                            photo: 'images/photo/hillary.png',
                            link: 'https://en.wikipedia.org/wiki/Hillary_Clinton',
                            description: 'Hillary Diane Rodham Clinton (born October 26, 1947) is an American politician and the nominee of the Democratic Party for President of the United States in the 2016 election.',
                            data: [
                                {
                                    plaintext: 'HLLR_C 423498433413484239545291023995351909531',
                                    encrypted: '114bce28c9b8c58ab8e3291fe91b3cfdbd0bceae7b6a5a692b5f76d9fa9a9381ef73d90967d4dfa27d5f437aa79f1bc6021bf38b7701ce969ae6560e49ef1323c5705290bdc8450697d40ae0abb6f665fbd010c2e3e225ebf0c0307748dd1320299c60d19a4a9e61d256a6170f19330e040c90f4c7dc1cc67b4567842e228c16',
                                    hash: 'cf6fe940cbb8585e57ded32a48ee60a9903641643db34c399ced743b0d5987d2',
                                    prefix: 'cf6fe940',
                                    memo: 'horse decorator belt',
                                    txLink: 'https://blockchain.info/tx/8b7eb45bffd8ff52759ff34334b32843025114ad0d9b5ef59dbc0da8d5c41a3d'
                                },
                                {
                                    plaintext: 'HLLR_C 231238453037934840923845086301230953083',
                                    encrypted: '27a9798118857c21b511b6c70b3cd93d00d25747c5a5cff644ac2739d9dadf6a951f1df02e67fef0583cf24589079d09937b5a8672f77288697b33941e95d5ce5a2dd182db331112673e18c1eca4b11d028518203111f694e523d582ee67cf5487f5ccf5725e1b0a8ce9a875fa3b361f9d3815602bc1ab6215268810ebfcbc04',
                                    hash: 'ad745dbace9e49a544669223b7540af37759df2dc56782a832968efb1c1d1c32',
                                    prefix: 'ad745dba',
                                    memo: 'schedule salt aquarium',
                                    txLink: 'https://blockchain.info/tx/0a7f313c9d486a13b7983a190528742c7a16f3b0fd433bb0c49af6a554a46aec'
                                },
                                {
                                    plaintext: 'HLLR_C 091834651835632984653290192302938665338',
                                    encrypted: '25fff4706730dad9f437028e8387e6d0505032bd9e08311fb50118ed1dcf80592412847237c15ce9a8bab938a92a709ffafa7c7807bd7b08aaa51e75dc1127c8e9782e82394eb740c231cdbc336f1aa0f70d0da2e18ff32f889802de3c911d7971bbb8eaf844e277df188e463703944999cbc907f76cc3cef59568e9fc02e2ce',
                                    hash: '1bb185c3080d48f938425785a9b456bda14cbddc2f771f771acf319afdf4335b',
                                    prefix: '1bb185c3',
                                    memo: 'forest passport clay',
                                    txLink: 'https://blockchain.info/tx/78e34962ebb2b4641061fbc271ed84d8a1c7358c9c1383ac30d1236c3e213a63'
                                },
                            ]
                        },
                        {
                            id: 3,
                            name: 'Candidate3',
                            code: 'CNDDT_3',
                            photo: 'images/photo/nophoto.png',
                            link: 'https://www.google.com',
                            description: 'Context candidate short profile. context candidate short profile. contextcandidate short profile. contextcandidate short profile. contextcandidate short profile. context.',
                            data: [
                                {
                                    plaintext: 'CNDDT_3 42348701230490323190239532532590349243',
                                    encrypted: '30cd66550dc50bfc6e8bf85eaf7366dd1145dce5e7cb41ff4e05f23c3ae635ee9d7fb5a33b8a714ccb2ea2c328be0da37f6d799d41d21ad95bb9eb4cb722f5e6cefda5074a76d73e67da92b97970d185ed2b982a9a7f010353132ad5df6338f113fec4cbafb7fadaf35b7ab7a41ff98ee424adf2e189ce1450a0ad8fdc40166e',
                                    hash: '6200d521c30d21f0d75a86baa518ba347a750a8926ae444583b06bfd790bb06d',
                                    prefix: '6200d521',
                                    memo: 'soccer sign lens',
                                    txLink: 'https://blockchain.info/tx/0afe832c897f6e1c8dd005b5cf14ffce1c0cdb254080ddcbaab1dbda66b90682'
                                },
                                {
                                    plaintext: 'CNDDT_3 09432758376549312909123236539379244729',
                                    encrypted: '7d7b77f9214b6304f9f2fe1fde7797b0118eb64d0eb9a936acbb01320016cb7ac4e3d1ee3bc2010616908ef5cbb77992665ad79958d9d945d49d6927390cd459ebc8072fd427a30f7e0d46be23a6abfee57c3a655320d7ea393264d3fefa92d43b7fefdce798fca164a3953a8e9fe317ad35d9de2bd5c53136b17fdd3de8f028',
                                    hash: 'd9dd36475c679a4f5862efdf82d339d19f0107082388cb50386e594fb2ddf862',
                                    prefix: 'd9dd3647',
                                    memo: 'manure reward ravioli',
                                    txLink: 'https://blockchain.info/tx/ef99575f7ab81c24f21f0567042be2d103d3e9b2223a8501e9b92fa253a46807'
                                },
                                {
                                    plaintext: 'CNDDT_3 18743944549847548709097234712365329348',
                                    encrypted: '55993a866794e6e4309b9c53f68b18888b42dd1544102010b24f8f84ecf7eda720dbaff6d0665edb14f0346e16781769dae66f6d3deb201d9f8ccd4a9efb3ddf6eac27b8625226c936d8b9533b52fdd3a46a12c4432593b88cdbefcca39e9091ff61a1711ea6369e56275f8de5ff66e486ffa0e2e543d9908569a88f451b2f5f',
                                    hash: 'ba9f48114e9639ea01048144b9ae6cf3c2a9385777b697b3dd6ba3ea83293aca',
                                    prefix: 'ba9f4811',
                                    memo: 'bishop pride statue',
                                    txLink: 'https://blockchain.info/tx/fa623423ae3d15ac9ab42a0ac67248789489a3a91f88ef6a5f649378b26edbed'
                                },
                            ]
                        },
                        {
                            id: 4,
                            name: 'Candidate4',
                            code: 'CNDDT_4',
                            photo: 'images/photo/nophoto.png',
                            link: 'https://www.google.com',
                            description: 'Context candidate short profile. context candidate short profile. contextcandidate short profile. contextcandidate short profile. contextcandidate short profile. context.',
                            data: [
                                {
                                    plaintext: 'CNDDT_4 22742665987934981239097437748928745302',
                                    encrypted: '4a38d8675e5b97433e87c3c081d8549fc35317011b4e380640c8455cd45074fe51d28b11feaf223ed787dafccc3feb31f2aa13ccba148cebbd388a68499093c663af3fdec7a5581e8a35f5d36be1259f2cabfcbeeb48a02f4e5037f7da9c1b4b8222d8b953ecc8f1bf2ace994872c48363e0b35891d09ae47cfe72ac7ad505a',
                                    hash: 'aaaf9635d408fce8534cf68ccb8a97c32e27b5397030a86a7b1086fbf9351e79',
                                    prefix: 'aaaf9635',
                                    memo: 'cat nest angle',
                                    txLink: 'https://blockchain.info/tx/22e171aabe8f27b1a8dd42eddfbc4fbddcb9b54feb862c6b721114407b57e996'
                                },
                                {
                                    plaintext: 'CNDDT_4 12382346106693469238643896932586001231',
                                    encrypted: '7ab7ede3d8cef23ab0b951de3f27917287524ff75d7103220e5d09ce578192e74410bfad021bce1fdb725e04a9f25ddc81e5208358750643675f377d791dcad992626e6fb0c1036105e8d5416de3a3ed54c06ac9d076371b92709f7fdc551660d11231dd9c25606edb987dd5893daca71abd17d1c1dd8eebef94cea75313272a',
                                    hash: '6362b35518f6e05ecca5d30358a2212c06aa175117a349c6dc4d8259da46ffb3',
                                    prefix: '6362b355',
                                    memo: 'waiter lemonade magazine',
                                    txLink: 'https://blockchain.info/tx/610ec2b917435b444ae7eb3f2727fa8dd9fdbbe7d0e28a079b9b0f8b12a6cd3b'
                                },
                                {
                                    plaintext: 'CNDDT_4 12309923189743276437346267862387002145',
                                    encrypted: 'c550e95d4d82e694f9bdd26f7976942cdc1e463e122404ad50db77388897a166ed784394354ec95e18d33a1f966fe15c08068831e6396392930a0138d6cf251fbfc55edd1b493c4c648147908f7f8318f8f833291893fdf9b6844631d9750fcf356464189488f3abf29d798954c52cf96bfed3101f42040871da18b0f4df36ed',
                                    hash: '2f7e83269b6079bbd84229b3c51ba6dfb8b1d13fcc3bb38c2192f258bfd5b2e3',
                                    prefix: '2f7e8326',
                                    memo: 'shrimps pail copyright',
                                    txLink: 'https://blockchain.info/tx/8cc74e997781b5e612dc0c58b0fd3874c75e9f70b4507d3128f3f748bbcce9c9'
                                }
                            ]
                        }
                    ]
                },
                {
                    id: 2,
                    name: 'Estonian Presidential Election',
                    type: 'Head of State Elections',
                    ends: 'Mon, 29 Aug 2016 23:00:00 +0300',
                    description: 'An indirect presidential election is scheduled to take place in Estonia on August 29, 2016. Incumbent President Toomas Hendrik Ilves, having served the maximum two terms, is not eligible to run for re-election.',
                    modulusHexadecimal: '62fcd5b24c6997541290c17e6efd232ca6572285f69477ffcfa533e99115e4854f0b4f02747d359faf230b0f92da306ce34debc71eab5dbf0c0105ee894a7f345ea165b69207eba75baf41ff117456d6085a1ca03e48753e6e399e8c6031e786bab0c08bb865af55eaa0ac833ba6b0f933dbe1433ecbc4ed1d026662c56b9f8d',
                    publicExponentHexadecimal: 10001,
                    candidates: [
                        {
                            id: 1,
                            photo: 'images/photo/est_1.png',
                            link: 'https://www.facebook.com/joksallar/about/?entry_point=page_nav_about_item&tab=page_info',
                            name: 'Allar Jõks',
                            code: 'LLR_JKS',
                            description: 'Estonia`s Pro Patria and Res Publica Union (IRL) on Sunday elected barrister and former chancellor of justice Allar Joks as its official candidate for the position of president of the Republic of Estonia.',
                            data: [
                                {
                                    plaintext: 'LLR_JKS 092183109345097463470201382366529863493',
                                    encrypted: '2a2d8c2c2f77a3e4e442cd6353a3d68615c78b7b08456eb6f2a9842fb2adb7ac94ff6804749beace2c8fc218e2ed7be3aa5531bbd331c3fa6c8adb6fcbcc366f2410f13a88aa9e02672974ba158102807dcd99cd9a21c05a298dee0cce57bcd815f1a1ea59beca84181454a119aa93f6147f98fb798e39444abd7449608874da',
                                    hash: '6bb3732abcc5c60bd836faf700d6482d50ddea41688531f029032542d0c8c6e2',
                                    prefix: '6bb3732a',
                                    memo: 'toothbrush revenge senator',
                                    txLink: 'https://blockchain.info/tx/0afe832c897f6e1c8dd005b5cf14ffce1c0cdb254080ddcbaab1dbda66b90682'
                                },
                                {
                                    plaintext: 'LLR_JKS 219371034034234698654092204903943748641',
                                    encrypted: '472e47e2cff4b758c2d74de9c3de9a71d77f298482a02c6b5e8e0329b0c0c6a8afd45e2c2ac589465c46f278ca7a5a6dc57a916a90cf1e7928f696fb345a60fb8c1f2a177a2a8e852b30418f6515c58a82d291a4151a9d578cf21e23ec122778d9c58e51b0329170a1f3083892940a9f9a0abb28aeaa94ffb1b59ffdcb87989c',
                                    hash: '0226a00d20d89266533a9a6f42c3d1241cb295d282a140ed07c121a8feaf5847',
                                    prefix: '0226a00d',
                                    memo: 'parlour strategy loop',
                                    txLink: 'https://blockchain.info/tx/ef99575f7ab81c24f21f0567042be2d103d3e9b2223a8501e9b92fa253a46807'
                                },
                                {
                                    plaintext: 'LLR_JKS 328479778237494063003995400334994485984',
                                    encrypted: 'ea20e1aa877d8bc7bfd24f3699a94a91b9ba3cabe13496ed173f1481ea979e535d90b7ad33079de2c251625fe5ac2db2db2e754c57d8f3d8ad0424b6a83f3c006567a7bf260f5ae32828926e4e101fd7cfb1e0763f825549541e8973974eea5ce3149a0c92c0e56b1376b1a88a58582e13a01bd7e71bfcd369bc8c26a2a97af',
                                    hash: '4eee7d5494cc703a160d730abcef15c39f4ae99f4e5f68a7e0e0915fd6dca029',
                                    prefix: '4eee7d54',
                                    memo: 'lover saw robot',
                                    txLink: 'https://blockchain.info/tx/fa623423ae3d15ac9ab42a0ac67248789489a3a91f88ef6a5f649378b26edbed'
                                }
                            ]
                        },
                        {
                            id: 2,
                            photo: 'images/photo/est_2.png',
                            link: 'https://en.wikipedia.org/wiki/Siim_Kallas',
                            name: 'Siim Kallas',
                            code: 'SM_KLLS',
                            description: 'Siim Kallas (born 2 October 1948 in Tallinn) is an Estonian politician, who most recently served as European Commissioner for Transport between 2010 and 2014.',
                            data: [
                                {
                                    plaintext: 'SM_KLLS 90171047897217978234978430399348983123',
                                    encrypted: '28a0297d61374119140949ee05c8c0a203632395e9037f3468aa9fba9cfd300915732dda367a821dd43f33705996b4976cecc6314fa8254eec5343435c9e4b006323d1b3c0ba6f374fa53472d048a5f09bc3f5d618c4ca16ea862b2044330e592c4caaac9b22958fb0938a03ca71186ffa5a5b85c15f18dae934d1e60178451e',
                                    hash: 'e804f7b0f4e5189336b4917881471cdf26e3d106d64822a11bf7842667099b9c',
                                    prefix: 'e804f7b0',
                                    memo: 'peacock horn window',
                                    txLink: 'https://blockchain.info/tx/22e171aabe8f27b1a8dd42eddfbc4fbddcb9b54feb862c6b721114407b57e996'
                                },
                                {
                                    plaintext: 'SM_KLLS 13898012035287346683423059645898424430',
                                    encrypted: '69f73d362a2d1f1a68721681bf7807e46fe402a7deb982c4ee2cb08641d0b69a57f05342bcfb7a3c413d698134e443cac87fe73c3532cebdb35ea114a51c75d7bb46f2320599a0cdf894c298362d1633f992979aad83880255cbb8e6d0efc0891441adda9503a85b65ac57b57d88919ef519005959e3c7e93fc201ddbf68031',
                                    hash: '8c1875dab0139601ac1f7b835490d5e6012bea613827407c966205fc5260b00f',
                                    prefix: '8c1875da',
                                    memo: 'frock volcano bill',
                                    txLink: 'https://blockchain.info/tx/610ec2b917435b444ae7eb3f2727fa8dd9fdbbe7d0e28a079b9b0f8b12a6cd3b'
                                },
                                {
                                    plaintext: 'SM_KLLS 09281029843272398566239824277823499320',
                                    encrypted: '355b715540d57da869804de7c2b4c471a22d2a9148d5bd49569a68206ff7bc53542ffba68d2b635f4f72ba23c03ae1462d33a5a4ef900d44c50e358f283f6fd7da94f0e9439ab9e811f46dc2a55064ce239f9099954e4e471f89526a8d0a09cdb37ddd690cee7b05a153fe8b8965d35e9cdfe625594ff4d4d467a02413a01d29',
                                    hash: 'e050ac4ea6784eb59acc86babbffa9c72524f716e5ed39d210a31f58878870b3',
                                    prefix: 'e050ac4e',
                                    memo: 'mask statue salt',
                                    txLink: 'https://blockchain.info/tx/8cc74e997781b5e612dc0c58b0fd3874c75e9f70b4507d3128f3f748bbcce9c9'
                                }
                            ]

                        },
                        {
                            id: 3,
                            photo: 'images/photo/est_3.png',
                            link: 'https://en.wikipedia.org/wiki/Eiki_Nestor',
                            name: 'Eiki Nestor',
                            code: 'K_NSTR',
                            description: 'Eiki Nestor (born 5 September 1953 in Tallinn) is an Estonian politician, member of the Social Democratic Party. He was the leader of the party from 1994 to 1996.',
                            data: [
                                {
                                    plaintext: 'K_NSTR 090931239078165109863248634626898493893',
                                    encrypted: '3326c3660a7c292485fd3d20daa67ac5d805a8c40dda9e1f79d5959de2e2a1d92ec905115cde58dd598d7b05040f3b55d4c8a41c1b32bc307cdfa17564e2f40e24bcb7a42427db33c82d3c18de24586925accce2ba67eb6b6e4087591060da65bb55632865ec0e76c6ada5a67132183fd1b1a7abb20454ffb9706d5b581e6e39',
                                    hash: 'f2796c763f9621b7a4468336f5101004932c5dc04b5c3a819e26483f0830fbc1',
                                    prefix: 'f2796c76',
                                    memo: 'dance violin screen',
                                    txLink: 'https://blockchain.info/tx/8b7eb45bffd8ff52759ff34334b32843025114ad0d9b5ef59dbc0da8d5c41a3d'
                                },
                                {
                                    plaintext: 'K_NSTR 143819873520986663894623476892389747438',
                                    encrypted: '43a2ae86be7ab6cffd6df00c1fa2edd3094df3945699c49786f94fb13e5cf1477c540aaaca5433171a2576c786bd7a4d40edfb8b88375a78d015281309e401f47462f4bd8cb40bf2fdc7f91340e0e0af1ec23d33a671b24eb196a0f7d3def07717587ec53c6bf2817c42b3430d805652f3b94adcfdb62d4693b0728adf50b86a',
                                    hash: '4e78dccba075d6e03386be865aed70677d2d1db32344469fdc1d71f139c43ed9',
                                    prefix: '4e78dccb',
                                    memo: 'end tractor advertising',
                                    txLink: 'https://blockchain.info/tx/0a7f313c9d486a13b7983a190528742c7a16f3b0fd433bb0c49af6a554a46aec'
                                },
                                {
                                    plaintext: 'K_NSTR 857489370279204234902973242709497230344',
                                    encrypted: '33c3d7b24371c3051d1f37d5255c9ca89e7af297598ead35f58c2058668ef703ba08b72600d87319367abe4d41b1469edb6ba79a6a8383f60fad3e94e51e50421e8a65e9d3bb7638bc0288b2627b8bfcde22d48fe9be9ff7dc7b3f7c48cf217108f812a51373d1307d05e575d622e6c6edab39ac52e29a5a1db50c4d4ef2ac91',
                                    hash: '057114f3e669a48b0db64ef01ace35d51cc3ca54794111db5525d3ba6e24460a',
                                    prefix: '057114f3',
                                    memo: 'work snack shoes',
                                    txLink: 'https://blockchain.info/tx/78e34962ebb2b4641061fbc271ed84d8a1c7358c9c1383ac30d1236c3e213a63'
                                }
                            ]
                        },
                        {
                            id: 4,
                            photo: 'images/photo/est_4.png',
                            link: 'https://en.wikipedia.org/wiki/Mailis_Reps',
                            name: 'Mailis Reps',
                            code: 'MLS_RPS',
                            description: 'Mailis Reps is an Estonian politician, a member of the Estonian Centre Party. She served as the Minister of Education and Research from 2002 to 2003 and from 2005 to 2007.',
                            data: [
                                {
                                    plaintext: 'MLS_RPS 09812398232772349274539723790923742732',
                                    encrypted: '53df6028655380f84ba73116efddb25ea7e38c556d1f699bfa9396772adb59a6c51ed95d369bed4a7b5af8af1d748271e196b695266e1b07c348818d5197ada57a3909ca4dcf5bdd63fa046d2e0a4af743a6d318906162622cff5960c96101c9298c9651d4e9fb43e8ffd01e08b20c546575b9265cfff5660f2756021d6e30dd',
                                    hash: '813e4523492b286b2742c0b5ca8ca908e8945ae82e4fa549a4a7107c3752cd23',
                                    prefix: '813e452',
                                    memo: 'bikini smoke colony',
                                    txLink: 'https://blockchain.info/tx/e19acca2b198c3fa85ee2b65839bb0a1ce9d49fd7ad43f5ae13222ab304011a3'
                                },
                                {
                                    plaintext: 'MLS_RPS 83109283873453896836426373489734902391',
                                    encrypted: '2d4769c245626e2aca50cb3f6eb9dd741c014eefee294fcb07743e15438468d5f3cc8ff304fd425690d35f9b767f0d0ea45a9d18a63a50dd2d382e24f79e3a687ceb2c4777021b7208fa55b66d379293c236fe0dc4a68c8ef61e39fdfa38687d9b7e21c9dd0406caf74911362ef4faf383cb84f8ce89d90ec0b1db1132f229f4',
                                    hash: '6e14a1a1ebc8c368e3da16c035eedc1c80b626131183937be3046eb908b50ff6',
                                    prefix: '6e14a1a',
                                    memo: 'kangaroo stool bikini',
                                    txLink: 'https://blockchain.info/tx/17a40f72f52e9ea317467b0ffe622153508b7b9b937b3931eced01cda6e9a403'
                                },
                                {
                                    plaintext: 'MLS_RPS 09403499752896581288966322983439989844',
                                    encrypted: '35140d56e30c13c85c50137e563447dd71351d55a2726136da36256b662058f41188dfaf5b7f6c4b1e1ccc1e01076a43752c008b43d0c5df405b16d81e079a1e5e31acda38ed5df109f3b7e9ffee7dc5fb6fe2be31888a880eb73759f0f8990cc544e7541e41de7fe04c4a841ed7075d1823020c0f70d63ba0db585c7842c570',
                                    hash: '5bc00277152f8eb76d59904760980c0610d016320925bb6c56a234afef70c2be',
                                    prefix: '5bc0027',
                                    memo: 'card stomach prejudice',
                                    txLink: 'https://blockchain.info/tx/a17def12f90505574bca79799b3a1aca9828661626e9ea534d58d01f0fb7e653'
                                }
                            ]
                        }
                    ]
                }
            ],
            monitors: [
                {id: 1, name: 'Monitor ongoing elections'},
                {id: 2, name: 'View finished elections` results'}
            ]
        });
})();