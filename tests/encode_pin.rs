mod common;

use std::collections::HashMap;
use std::fmt::Write as _;

use common::{corpus, corpus64};
use leake::encode;
use leake::format::{Quality, VtfFormat};
use leake::frame::Frame;
use sha2::{Digest, Sha256};

const DIGESTS: &[(&str, &str)] = &[
    (
        "noisy1x1/RGBA8888/SLOW/dither=false",
        "f02240c8a0f8b717539f2dc9cc8c8f5224480cbf0c9cb6759abb434ee3fd2283",
    ),
    (
        "noisy1x1/RGBA8888/SLOW/dither=true",
        "f02240c8a0f8b717539f2dc9cc8c8f5224480cbf0c9cb6759abb434ee3fd2283",
    ),
    (
        "noisy1x1/RGB888/SLOW/dither=false",
        "a6bff0c1eb666b1a338e99909f596f62bcd99e647c51d5b0d3f1abd573e457ed",
    ),
    (
        "noisy1x1/RGB888/SLOW/dither=true",
        "a6bff0c1eb666b1a338e99909f596f62bcd99e647c51d5b0d3f1abd573e457ed",
    ),
    (
        "noisy1x1/RGB565/SLOW/dither=false",
        "e4e9d16a3d8880b718cc0a9f7b4109acd83c059ec1eb67239ce9bba7a0091fd8",
    ),
    (
        "noisy1x1/RGB565/SLOW/dither=true",
        "38124f38ae0674a410f47d161b96764893097e551ddc8781e563b183391b7074",
    ),
    (
        "noisy1x1/BGRA4444/SLOW/dither=false",
        "5c6369e3b3adc3752e7cc39537ff50cb3f787104fbf36a9f76031799770cd173",
    ),
    (
        "noisy1x1/BGRA4444/SLOW/dither=true",
        "858d9b9b968f4cf4089c17caeea2e72be54ea62970da6ce1a37e4f78e70e74d9",
    ),
    (
        "noisy1x1/BGRA5551/SLOW/dither=false",
        "dbfeff6ead0d1ed89eb1e28cd11916a814364e719d94a812f9f14f2e53220863",
    ),
    (
        "noisy1x1/BGRA5551/SLOW/dither=true",
        "ff48eb4f2d6ccc6e729155ea9839b448fad5930f9ddba688c2e02dc2be7b618e",
    ),
    (
        "noisy5x5/RGBA8888/SLOW/dither=false",
        "4dfc98a0cc5b5a832fe8809eb0c6d26394a6f7f7631388accaba1582533d4847",
    ),
    (
        "noisy5x5/RGBA8888/SLOW/dither=true",
        "4dfc98a0cc5b5a832fe8809eb0c6d26394a6f7f7631388accaba1582533d4847",
    ),
    (
        "noisy5x5/RGB888/SLOW/dither=false",
        "d46923eabe2b1179e0fd5181f9cc0762d84addbf4a2ae1180891e53b224b7fe7",
    ),
    (
        "noisy5x5/RGB888/SLOW/dither=true",
        "d46923eabe2b1179e0fd5181f9cc0762d84addbf4a2ae1180891e53b224b7fe7",
    ),
    (
        "noisy5x5/RGB565/SLOW/dither=false",
        "59b6050f1a4d74b074445ce2ad90faa372232d412ffd7d1dbf2912ccb79324e8",
    ),
    (
        "noisy5x5/RGB565/SLOW/dither=true",
        "40423e8e31f5bf8b06adc5154f62884a12958d4388fe8c5ba8526cef18c4ebce",
    ),
    (
        "noisy5x5/BGRA4444/SLOW/dither=false",
        "466586c938ee0b1bbb9fd6b311097fcc290b0678a58f555b38f946f56ea65555",
    ),
    (
        "noisy5x5/BGRA4444/SLOW/dither=true",
        "46238c8452ea045fc886d10e31571f5b828875839e3b028bebec6aa3c06b0b04",
    ),
    (
        "noisy5x5/BGRA5551/SLOW/dither=false",
        "5cfcc87281d665e5d69ba71227dd77cd446c8252f2298826b784a0809a2a857b",
    ),
    (
        "noisy5x5/BGRA5551/SLOW/dither=true",
        "6af041d71793602cb2f3a28cda546dbfccbb9dafc1a7bd3a9ab23b85911052e7",
    ),
    (
        "noisy8x8/RGBA8888/SLOW/dither=false",
        "e3cb5296657129d0fd74082470e744de98250ac524d6f4c28c928c9c644c2f09",
    ),
    (
        "noisy8x8/RGBA8888/SLOW/dither=true",
        "e3cb5296657129d0fd74082470e744de98250ac524d6f4c28c928c9c644c2f09",
    ),
    (
        "noisy8x8/RGB888/SLOW/dither=false",
        "67bcf25a7dcbfee3c2f6c577811ecc82bb4cfc24841bd270bd06afafe4d06580",
    ),
    (
        "noisy8x8/RGB888/SLOW/dither=true",
        "67bcf25a7dcbfee3c2f6c577811ecc82bb4cfc24841bd270bd06afafe4d06580",
    ),
    (
        "noisy8x8/RGB565/SLOW/dither=false",
        "6e30e3309d22d3733f4df840c69f123c8e9c4dcb439d223af2c068a89e233c30",
    ),
    (
        "noisy8x8/RGB565/SLOW/dither=true",
        "87719d84548fb139dc7fb8df2294b12e1b21d8f815500f1fe1f4c47dae8e8ddf",
    ),
    (
        "noisy8x8/BGRA4444/SLOW/dither=false",
        "f8a945d3214de77b4e0af253c58a5be8121c593fd8129bdd9fff7e6c8d9460dc",
    ),
    (
        "noisy8x8/BGRA4444/SLOW/dither=true",
        "8a80de600df1a87688ad95099a18f2c7042f3ae77bc04f1312cacf81c2fe1737",
    ),
    (
        "noisy8x8/BGRA5551/SLOW/dither=false",
        "fcbc05299b27a3934cb4faaa1eea98a03e07d1f733a00b6498beee7e94adab65",
    ),
    (
        "noisy8x8/BGRA5551/SLOW/dither=true",
        "ad7fc7673eec9717d3b71aac9b4b0afad4fb6d40ec0b24e707b553942b897c62",
    ),
    (
        "noisy20x12/RGBA8888/SLOW/dither=false",
        "6616d44ba86312d7e17092bcb869acc0f9d9c874d67fec4c10135ddaf169f90a",
    ),
    (
        "noisy20x12/RGBA8888/SLOW/dither=true",
        "6616d44ba86312d7e17092bcb869acc0f9d9c874d67fec4c10135ddaf169f90a",
    ),
    (
        "noisy20x12/RGB888/SLOW/dither=false",
        "2e4e52b38e7b1b0ba3c4615b03654c06bfa8577bcf7ad78e2bad196d98f8add6",
    ),
    (
        "noisy20x12/RGB888/SLOW/dither=true",
        "2e4e52b38e7b1b0ba3c4615b03654c06bfa8577bcf7ad78e2bad196d98f8add6",
    ),
    (
        "noisy20x12/RGB565/SLOW/dither=false",
        "b20f4439c458213a606665e67ff282132ccae99679084f79a48fcb7083d547fa",
    ),
    (
        "noisy20x12/RGB565/SLOW/dither=true",
        "8a2b686123f81ad07e5490218d8f135c38a1ba50e1301400cca5389588f288ae",
    ),
    (
        "noisy20x12/BGRA4444/SLOW/dither=false",
        "84eca0dbf48eae5e2e68274008d15d5465901096d7a255481ab5c7c0eb2e08a4",
    ),
    (
        "noisy20x12/BGRA4444/SLOW/dither=true",
        "ffb57e110c733717df231648bd63a875b67f219368c69de2286bf9c34d974b80",
    ),
    (
        "noisy20x12/BGRA5551/SLOW/dither=false",
        "adfca9c682945e6b8ab47324664dd07d8a10793d445362cb46803d64c197c49b",
    ),
    (
        "noisy20x12/BGRA5551/SLOW/dither=true",
        "39f43c8771c33f0a15dc06f5d4a8078a630d75f92ec3f41c43bdcdfcbd2875ef",
    ),
    (
        "noisy64x64/RGBA8888/SLOW/dither=false",
        "4f62566a01ebf55634c9f805f8fa7fdb11d9e66042fc9c30324b36bb7c471e58",
    ),
    (
        "noisy64x64/RGBA8888/SLOW/dither=true",
        "4f62566a01ebf55634c9f805f8fa7fdb11d9e66042fc9c30324b36bb7c471e58",
    ),
    (
        "noisy64x64/RGB888/SLOW/dither=false",
        "fc95eb222cb1cd78b1e1b0ead11b2854f77df23297145f1f5e68918b90ca868e",
    ),
    (
        "noisy64x64/RGB888/SLOW/dither=true",
        "fc95eb222cb1cd78b1e1b0ead11b2854f77df23297145f1f5e68918b90ca868e",
    ),
    (
        "noisy64x64/RGB565/SLOW/dither=false",
        "353e2075e32aab710075607ce868ee06df09bbf44322aa68db04f1e942708462",
    ),
    (
        "noisy64x64/RGB565/SLOW/dither=true",
        "d52b93c3b6d0d5d20602fea4e4584ac70f391ecbcc44575089c339672d87896b",
    ),
    (
        "noisy64x64/BGRA4444/SLOW/dither=false",
        "0be85398be716a80de532d5952546015dbb3371e4b5ae161afe42f4626649b2e",
    ),
    (
        "noisy64x64/BGRA4444/SLOW/dither=true",
        "659a5f86f152947361f3576e9c98626af45e75e040977fee19d332ec5529b25d",
    ),
    (
        "noisy64x64/BGRA5551/SLOW/dither=false",
        "b6215fd5f8551fc3029858ab1961af18f47d69f61bc486a73fdd760289044631",
    ),
    (
        "noisy64x64/BGRA5551/SLOW/dither=true",
        "d71a0e059e98747a2f49c1edab885d6fe64b244de9f688a69dc36d0bec18d449",
    ),
    (
        "gradient64x64/RGBA8888/SLOW/dither=false",
        "27d6f64fd5e0867fa3661a6612e9312deb7de6ca741069390911951bad15ba94",
    ),
    (
        "gradient64x64/RGBA8888/SLOW/dither=true",
        "27d6f64fd5e0867fa3661a6612e9312deb7de6ca741069390911951bad15ba94",
    ),
    (
        "gradient64x64/RGB888/SLOW/dither=false",
        "f222e34b02a374db22c694974fc794a7769d08f19ccde3a853c93ffc4d88bb61",
    ),
    (
        "gradient64x64/RGB888/SLOW/dither=true",
        "f222e34b02a374db22c694974fc794a7769d08f19ccde3a853c93ffc4d88bb61",
    ),
    (
        "gradient64x64/RGB565/SLOW/dither=false",
        "6990c0842fdd56c7970462d616b6970d493cf68dedb0d488872328390b4984a9",
    ),
    (
        "gradient64x64/RGB565/SLOW/dither=true",
        "6de86ebac79856d673f5dc35962df04852e6a1e31bfb3a36dc3d2ba9abbc410e",
    ),
    (
        "gradient64x64/BGRA4444/SLOW/dither=false",
        "7503178f5e47eb0f7089906ec69c5139156e595a409c652530f2549227a0cdfd",
    ),
    (
        "gradient64x64/BGRA4444/SLOW/dither=true",
        "a430ebdc2324c67fa5587547b15d481f9b97a76808ad4376debea51f292120b3",
    ),
    (
        "gradient64x64/BGRA5551/SLOW/dither=false",
        "45f5e436c30f6f2a593ee461801fef7ec9fe99cba33f7f7e66db5a01d328b3a0",
    ),
    (
        "gradient64x64/BGRA5551/SLOW/dither=true",
        "1fd46962dbd40716a1fab01195e536d35ffe46887280e1a1785424bb15ec34ab",
    ),
    (
        "midAlpha8x8/RGBA8888/SLOW/dither=false",
        "e47319eeb447d753379834232e03ae3051bb6df66b0fdf63cd96c21a8d24b850",
    ),
    (
        "midAlpha8x8/RGBA8888/SLOW/dither=true",
        "e47319eeb447d753379834232e03ae3051bb6df66b0fdf63cd96c21a8d24b850",
    ),
    (
        "midAlpha8x8/RGB888/SLOW/dither=false",
        "86f175f0b78b9aa60f9d6013b0a3802a21ba210e80f0be5e6c75aefc1300b0e9",
    ),
    (
        "midAlpha8x8/RGB888/SLOW/dither=true",
        "86f175f0b78b9aa60f9d6013b0a3802a21ba210e80f0be5e6c75aefc1300b0e9",
    ),
    (
        "midAlpha8x8/RGB565/SLOW/dither=false",
        "2edf6238b912fcc80e9a9b7dc6bc6fbbebba91a061b93e5cd6b646b331f11c18",
    ),
    (
        "midAlpha8x8/RGB565/SLOW/dither=true",
        "edade41b871643fe5ff52c06cd2b599323b85f4c7625f9526886b587217a50bb",
    ),
    (
        "midAlpha8x8/BGRA4444/SLOW/dither=false",
        "2c442f8c33d85721b4e64412a91a561137ebca9b769266acc2a5fcc9aa026b47",
    ),
    (
        "midAlpha8x8/BGRA4444/SLOW/dither=true",
        "4fdbea734cdb780131b5f9b763dc4e55760e82e3e55e2093ba6e032eba4f24e0",
    ),
    (
        "midAlpha8x8/BGRA5551/SLOW/dither=false",
        "b16ac5d801572c63d6774e687d7bf127d0b576426276eaf03d8f7e3e709c2491",
    ),
    (
        "midAlpha8x8/BGRA5551/SLOW/dither=true",
        "2001279f49627198da21a5ec4c44dabb9981e0614abe1daa504d1fbbafc45c2a",
    ),
    (
        "halfClear64x64/RGBA8888/SLOW/dither=false",
        "5501b1eecd8e11535d7293e41696ac6b8f98fec97a514b736d8262e4745ec59d",
    ),
    (
        "halfClear64x64/RGBA8888/SLOW/dither=true",
        "5501b1eecd8e11535d7293e41696ac6b8f98fec97a514b736d8262e4745ec59d",
    ),
    (
        "halfClear64x64/RGB888/SLOW/dither=false",
        "00d1fbedc4fe5a3f17eb51700a0369cd0719779c81fa7348013f5cca66965ce7",
    ),
    (
        "halfClear64x64/RGB888/SLOW/dither=true",
        "00d1fbedc4fe5a3f17eb51700a0369cd0719779c81fa7348013f5cca66965ce7",
    ),
    (
        "halfClear64x64/RGB565/SLOW/dither=false",
        "2814a3fa786e06f9254a0173682887eeed1576d5d316895ac3b640013b34af65",
    ),
    (
        "halfClear64x64/RGB565/SLOW/dither=true",
        "1275a8fca5197ffc7bc6e86d949f4b3e9ad32f547cd05383879ad9c4b9b01d20",
    ),
    (
        "halfClear64x64/BGRA4444/SLOW/dither=false",
        "7e9ddebe264f87a00fd9212c209c17d3aeadf7f8bcada0cbed1547a52e582a7f",
    ),
    (
        "halfClear64x64/BGRA4444/SLOW/dither=true",
        "31efa0ecc0f81992817e5883a63f7d6a5b04ff96b19f1192c97714dff6778bf9",
    ),
    (
        "halfClear64x64/BGRA5551/SLOW/dither=false",
        "20e978763c5f6986aea053788566d1253036ac68d782cc138bc95b620d42488e",
    ),
    (
        "halfClear64x64/BGRA5551/SLOW/dither=true",
        "7f853d3d3561ab0cd684a201ae4d7bb93787dcea4eb5bb2abe2147550827d01e",
    ),
    (
        "noisy1x1/DXT1/FAST/dither=true",
        "a4b944e88464527602f5d7192b83d0ec5c682ec25c880cc706a9d83fbfde6b61",
    ),
    (
        "noisy1x1/DXT1/NORMAL/dither=true",
        "44523c130e9855b64081ae6bc73d3cd87074c6bd9795f09b2bcce031fcfa53ee",
    ),
    (
        "noisy1x1/DXT1/SLOW/dither=true",
        "e850377c844e231236e57bd21c138acb2fdf479b396dd58fb9d0253445c87d33",
    ),
    (
        "noisy1x1/DXT1/VERY_SLOW/dither=true",
        "1b1149f01c5a3344a1a8b591393172f5912eb2003dd5b462102ea6d0adfd1d8f",
    ),
    (
        "noisy1x1/DXT5/FAST/dither=true",
        "eb473a767f0d9aaba7ba4ad13c0f506ece625f385b731e08e045d5bdc966ed26",
    ),
    (
        "noisy1x1/DXT5/NORMAL/dither=true",
        "7505694cf92adaac8cead084cfdee0a847e85f535f36d22d901992962b10d9d4",
    ),
    (
        "noisy1x1/DXT5/SLOW/dither=true",
        "7505694cf92adaac8cead084cfdee0a847e85f535f36d22d901992962b10d9d4",
    ),
    (
        "noisy1x1/DXT5/VERY_SLOW/dither=true",
        "7505694cf92adaac8cead084cfdee0a847e85f535f36d22d901992962b10d9d4",
    ),
    (
        "noisy5x5/DXT1/FAST/dither=true",
        "ba5ea4f0a43fd0e2a76291ddd13963a2dfea1c04bc0fd4acf7143062e5245f68",
    ),
    (
        "noisy5x5/DXT1/NORMAL/dither=true",
        "d0a4975b9804b123b4f3686862fc9bdd746727f85e2ac444bae34a573aec7d98",
    ),
    (
        "noisy5x5/DXT1/SLOW/dither=true",
        "a8dcf10e6924cc647f96829b4344d3c01beb9ce8616faf089d105a952bfd4906",
    ),
    (
        "noisy5x5/DXT1/VERY_SLOW/dither=true",
        "a8dcf10e6924cc647f96829b4344d3c01beb9ce8616faf089d105a952bfd4906",
    ),
    (
        "noisy5x5/DXT5/FAST/dither=true",
        "16f5d26961418007ece0f9185fa966ae1739a9a71bf9534b1226f2f9b83df9df",
    ),
    (
        "noisy5x5/DXT5/NORMAL/dither=true",
        "22cbc2e779027eff137ac3f142fe345c981faddefe63134400456fa3dbd0d350",
    ),
    (
        "noisy5x5/DXT5/SLOW/dither=true",
        "0b67d9e0c54328c223c0075c586709e434471490feb8cf14919ce2faa9c63e2b",
    ),
    (
        "noisy5x5/DXT5/VERY_SLOW/dither=true",
        "0b67d9e0c54328c223c0075c586709e434471490feb8cf14919ce2faa9c63e2b",
    ),
    (
        "noisy8x8/DXT1/FAST/dither=true",
        "d4768123ab000d136a7bcafb91883172822a8d7cb5df9ea1a8a3cf5fa228a995",
    ),
    (
        "noisy8x8/DXT1/NORMAL/dither=true",
        "69ae5efa63e567a549128fb98b8f683519c4ff1d9853007f4df8a66c1cf35da5",
    ),
    (
        "noisy8x8/DXT1/SLOW/dither=true",
        "fa30eb2cc0f992c3038daf8c40e127cefee65b1c4185f96f5327663a924e51f8",
    ),
    (
        "noisy8x8/DXT1/VERY_SLOW/dither=true",
        "fa30eb2cc0f992c3038daf8c40e127cefee65b1c4185f96f5327663a924e51f8",
    ),
    (
        "noisy8x8/DXT5/FAST/dither=true",
        "e1c28083ec930033fa8645f820aa77980fb5ac1ff75cf6a2a32c21ba18a0e992",
    ),
    (
        "noisy8x8/DXT5/NORMAL/dither=true",
        "b7a80ab8abd9c62bb5413ea6beb2c06034844e1791e8d7b60cded7ca9da3d42c",
    ),
    (
        "noisy8x8/DXT5/SLOW/dither=true",
        "2850411946b07cb84cbf723beb3959d2b2f6747f447439f1bede412041740017",
    ),
    (
        "noisy8x8/DXT5/VERY_SLOW/dither=true",
        "2850411946b07cb84cbf723beb3959d2b2f6747f447439f1bede412041740017",
    ),
    (
        "noisy20x12/DXT1/FAST/dither=true",
        "f07c03dcc80d9034cc64083b365aa6ff2c63905f82d4e8c976cd167cb89b453c",
    ),
    (
        "noisy20x12/DXT1/NORMAL/dither=true",
        "c829c16e74dc203c0e9e71848b2202a8df0b92846e0159731d3ebfdfac34cf7a",
    ),
    (
        "noisy20x12/DXT1/SLOW/dither=true",
        "d79f83ed0b1ee9fa8a1af71cdfcadc10c9602729036ac69519145701deea243d",
    ),
    (
        "noisy20x12/DXT1/VERY_SLOW/dither=true",
        "3c1662426406365de245a4922b7f19c96982f8b3352086f9540b3ca96a90abee",
    ),
    (
        "noisy20x12/DXT5/FAST/dither=true",
        "a17bb2798daee8c327d469e440118d7f0ff79bc09053523116e7ec3977e04d03",
    ),
    (
        "noisy20x12/DXT5/NORMAL/dither=true",
        "6d6ca95512cc46857a73e159daf25be2aedb69ba93884f7d42421523fc5a8084",
    ),
    (
        "noisy20x12/DXT5/SLOW/dither=true",
        "97833051f33697bc7d8c7056eec097bea7c3a89add0e93fab788d07a7578c717",
    ),
    (
        "noisy20x12/DXT5/VERY_SLOW/dither=true",
        "1cc0b1f2dca93e1a4acd7f9760c88122e79bdc6ee4d61d500b78549e886519dc",
    ),
    (
        "noisy64x64/DXT1/FAST/dither=true",
        "4ae7b8f48566a181e5a434aa8f0babdc1e4d9051285a5e6bd629b9b5e216ff3a",
    ),
    (
        "noisy64x64/DXT1/NORMAL/dither=true",
        "83e8776832819966228b16a2bb21fcf12b65e7f725abeacfe3001ddba79e78a4",
    ),
    (
        "noisy64x64/DXT1/SLOW/dither=true",
        "e729162052b160e105c37199349c6695df0865f2dee79991326fd34aefe3e202",
    ),
    (
        "noisy64x64/DXT1/VERY_SLOW/dither=true",
        "f705e79adf92a11c915e43cb70847573b07a55c1e2655d8ded6935577d8750ca",
    ),
    (
        "noisy64x64/DXT5/FAST/dither=true",
        "45baf50a9c0ad2d103516c241ff439d60c6b885781ff09da5a8c18f68d544727",
    ),
    (
        "noisy64x64/DXT5/NORMAL/dither=true",
        "e1353146f4a020b5060c092ade0b8961c6a790a3d0310182a96ca44bcb2686c2",
    ),
    (
        "noisy64x64/DXT5/SLOW/dither=true",
        "bc548eceafa299695f040cab44989cf3e58631fc11a9a10b186480bebbc6cee9",
    ),
    (
        "noisy64x64/DXT5/VERY_SLOW/dither=true",
        "599ceb99bb41f3dee857fdae10254d6ff6e11b4214ae6d2bbd7c02f99ad9f4a6",
    ),
    (
        "gradient64x64/DXT1/FAST/dither=true",
        "2d90a984fa5a7d5507f0fdc22f4a05d2bc2216ddb3e4cc286899e7521df5812b",
    ),
    (
        "gradient64x64/DXT1/NORMAL/dither=true",
        "e823f2a5c45ce8358fa886c9b7b879c76faf3066c277891af456a4e3bfd54e0e",
    ),
    (
        "gradient64x64/DXT1/SLOW/dither=true",
        "8f7fb42fab6687468ae66fdee4ed46498e566051c71a4118d88dafe827a730b8",
    ),
    (
        "gradient64x64/DXT1/VERY_SLOW/dither=true",
        "4f29f46846e9ba6374ff8ed7b7ba3c784fc6aa1bc925d6327c18f2b7fefeb312",
    ),
    (
        "gradient64x64/DXT5/FAST/dither=true",
        "9ea635289ca9caf25dcb75a48c61949fae1b66a78c0614e5e87fbc92b4135251",
    ),
    (
        "gradient64x64/DXT5/NORMAL/dither=true",
        "19d93d6f9aab6dea2914a9d440a79fc0710f8ebcd6e359d299a3a86bf7643451",
    ),
    (
        "gradient64x64/DXT5/SLOW/dither=true",
        "55a0078c499ce68369e58357a6913da4d429cf6ab1997686952c6b51d0b46556",
    ),
    (
        "gradient64x64/DXT5/VERY_SLOW/dither=true",
        "1fa8d1439dd78b92804cd75056e007b5fd9509d8e93041e0e9f7fbc871e21c3c",
    ),
    (
        "midAlpha8x8/DXT1/FAST/dither=true",
        "202901d96a2808a50b904386f5629971c85b530ef847542c2c33af2e1c74ec67",
    ),
    (
        "midAlpha8x8/DXT1/NORMAL/dither=true",
        "34ea2e38bd41f7165268c04dedd276294a567a838fb213785fa0b60fae632bd3",
    ),
    (
        "midAlpha8x8/DXT1/SLOW/dither=true",
        "7bfa88d4b1c434682315d004de3421e0c9bfcb4f527f2fc4893553bd2149497f",
    ),
    (
        "midAlpha8x8/DXT1/VERY_SLOW/dither=true",
        "9b74895ca1e99455245aae9a62a0b8bdc95214a98c0ede3225d53446139fc869",
    ),
    (
        "midAlpha8x8/DXT5/FAST/dither=true",
        "052da7a779d4b04455feb94ce981db1575550bb35c84044298c74af0c2406701",
    ),
    (
        "midAlpha8x8/DXT5/NORMAL/dither=true",
        "aa9c78b23b5e3ee0ddb53e731873942f8aadc86549347bc33c44d98e580f55c6",
    ),
    (
        "midAlpha8x8/DXT5/SLOW/dither=true",
        "addb9fb3086afad010a7f2c2c243c42455e4ea5efae437a579e88397a031b6a3",
    ),
    (
        "midAlpha8x8/DXT5/VERY_SLOW/dither=true",
        "b3df9cbefeba81405d57126f801a1fb9e3853560e8cf71ef4334a8f617697f04",
    ),
    (
        "halfClear64x64/DXT1/FAST/dither=true",
        "685a7888ac5ca0bb2e4846d069215daf89bf1049eebd557d6db10c6eda60c26b",
    ),
    (
        "halfClear64x64/DXT1/NORMAL/dither=true",
        "bebf824dd0036a986577aefe4b2b08cd78d00c323a3bcc1708b4a22b95c7d39f",
    ),
    (
        "halfClear64x64/DXT1/SLOW/dither=true",
        "db9c929c15b80690312ee8003d04c57132de62e176e08d4c3ded466ea5f2343d",
    ),
    (
        "halfClear64x64/DXT1/VERY_SLOW/dither=true",
        "9add370060d57c20fcb301fd0cc31f1d6bf205b1f7be2801790408002512051c",
    ),
    (
        "halfClear64x64/DXT5/FAST/dither=true",
        "bfc8aaebe32c48678ffd91989f796d5ab584f3d2ca9c8b43fc5971b64c447bfd",
    ),
    (
        "halfClear64x64/DXT5/NORMAL/dither=true",
        "860224e78ee0e767ac2ffd2e5d08bc69b4f11b86caa8b14894ebf5019f40f072",
    ),
    (
        "halfClear64x64/DXT5/SLOW/dither=true",
        "f39ca6439a9ae7a7a5339a79c050382522aa26ada124fcc3056e555aa3602dd2",
    ),
    (
        "halfClear64x64/DXT5/VERY_SLOW/dither=true",
        "4c62db883185c617f524a17a848f354ae70acf00157767d81a129e454825c8ca",
    ),
    (
        "clear8x8/RGB565/SLOW/dither=false",
        "38723a2e5e8a17aa7950dc008209944e898f69a7bd10a23c839d341e935fd5ca",
    ),
    (
        "clear8x8/RGB565/SLOW/dither=true",
        "38723a2e5e8a17aa7950dc008209944e898f69a7bd10a23c839d341e935fd5ca",
    ),
    (
        "clear64x64/RGB565/SLOW/dither=false",
        "9f1dcbc35c350d6027f98be0f5c8b43b42ca52b7604459c0c42be3aa88913d47",
    ),
    (
        "clear64x64/RGB565/SLOW/dither=true",
        "9f1dcbc35c350d6027f98be0f5c8b43b42ca52b7604459c0c42be3aa88913d47",
    ),
    (
        "flat64x64/RGB565/SLOW/dither=false",
        "59311ad03e3df1d6f2883d7fcfece0a4693d7b205c74e08b267fd2fdedb58452",
    ),
    (
        "flat64x64/RGB565/SLOW/dither=true",
        "e1ac11d66a765fd4bf5832c7bd1453d535b66517f165060bbc1bfdd1db2d921e",
    ),
    (
        "clear8x8/DXT1/SLOW/dither=true",
        "a40bbe62982dac39d5439f85fef02b5a6f670a1c0c0a21fa20593a3071abc7f8",
    ),
    (
        "clear8x8/DXT1/VERY_SLOW/dither=true",
        "a40bbe62982dac39d5439f85fef02b5a6f670a1c0c0a21fa20593a3071abc7f8",
    ),
    (
        "clear8x8/DXT5/SLOW/dither=true",
        "515e501bc3440ca06ea4abb8e4fcc82435bac15b46c3892b14c4b4aed9245990",
    ),
    (
        "clear8x8/DXT5/VERY_SLOW/dither=true",
        "515e501bc3440ca06ea4abb8e4fcc82435bac15b46c3892b14c4b4aed9245990",
    ),
    (
        "clear64x64/DXT1/SLOW/dither=true",
        "3995880593b7bb0204bac758c501ad1b3c27a4c2619cfa6c2dd04fc6d39dd599",
    ),
    (
        "clear64x64/DXT1/VERY_SLOW/dither=true",
        "3995880593b7bb0204bac758c501ad1b3c27a4c2619cfa6c2dd04fc6d39dd599",
    ),
    (
        "clear64x64/DXT5/SLOW/dither=true",
        "cfbc990253c70cf5b25824705d2967e7f07a163acf905b1acc326aa52ff58763",
    ),
    (
        "clear64x64/DXT5/VERY_SLOW/dither=true",
        "cfbc990253c70cf5b25824705d2967e7f07a163acf905b1acc326aa52ff58763",
    ),
    (
        "flat64x64/DXT1/SLOW/dither=true",
        "97a61a4448baf23ac168b378b8f03601a61086ac6e9aff57ece2c8a6e4d6b03c",
    ),
    (
        "flat64x64/DXT1/VERY_SLOW/dither=true",
        "72406cdd9ae6961c291094b0eb239afdb654bfc4fffca82da88fdc8a8d3ee34d",
    ),
    (
        "flat64x64/DXT5/SLOW/dither=true",
        "121a8b31df6ef725478e5b68ac855566061dbaa7b8a373cae656c57207bcd3f9",
    ),
    (
        "flat64x64/DXT5/VERY_SLOW/dither=true",
        "e3145b2c5e47885f8c6f0822acd8ae712f3d34cdba3694c2120ec359412a0ed4",
    ),
];

const PACKED_FORMATS: [VtfFormat; 5] = [
    VtfFormat::Rgba8888,
    VtfFormat::Rgb888,
    VtfFormat::Rgb565,
    VtfFormat::Bgra4444,
    VtfFormat::Bgra5551,
];

const DXT_FORMATS: [VtfFormat; 2] = [VtfFormat::Dxt1, VtfFormat::Dxt5];

const QUALITIES: [Quality; 4] = [
    Quality::Fast,
    Quality::Normal,
    Quality::Slow,
    Quality::VerySlow,
];

const DEGENERATE_FRAMES: [&str; 3] = ["clear8x8", "clear64x64", "flat64x64"];

const DEGENERATE_FORMS: [(VtfFormat, Quality, bool); 6] = [
    (VtfFormat::Rgb565, Quality::Slow, false),
    (VtfFormat::Rgb565, Quality::Slow, true),
    (VtfFormat::Dxt1, Quality::Slow, true),
    (VtfFormat::Dxt1, Quality::VerySlow, true),
    (VtfFormat::Dxt5, Quality::Slow, true),
    (VtfFormat::Dxt5, Quality::VerySlow, true),
];

const JOB_CASES: [(VtfFormat, Quality, bool); 4] = [
    (VtfFormat::Dxt1, Quality::Slow, true),
    (VtfFormat::Dxt5, Quality::Slow, true),
    (VtfFormat::Rgb565, Quality::Slow, true),
    (VtfFormat::Rgb565, Quality::Slow, false),
];

const VERY_SLOW_JOB_CASES: [(VtfFormat, Quality, bool); 3] = [
    (VtfFormat::Dxt1, Quality::VerySlow, true),
    (VtfFormat::Dxt5, Quality::VerySlow, true),
    (VtfFormat::Rgb565, Quality::VerySlow, true),
];

struct PinCase {
    frame_name: &'static str,
    format: VtfFormat,
    quality: Quality,
    dither: bool,
}

impl PinCase {
    fn key(&self) -> String {
        key(self.frame_name, self.format, self.quality, self.dither)
    }
}

fn key(frame_name: &str, format: VtfFormat, quality: Quality, dither: bool) -> String {
    format!(
        "{frame_name}/{}/{}/dither={dither}",
        format.name(),
        quality.name()
    )
}

fn digest(
    raster: &Frame,
    format: VtfFormat,
    quality: Quality,
    dither: bool,
    jobs: usize,
) -> String {
    let bytes = encode::encode(raster, format, quality, dither, jobs, None);
    let hash = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(hash.len() * 2);
    for byte in hash {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn ordinary_frames() -> Vec<&'static str> {
    corpus()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| !DEGENERATE_FRAMES.contains(name))
        .collect()
}

fn packed_cases() -> Vec<PinCase> {
    ordinary_frames()
        .into_iter()
        .flat_map(|frame_name| {
            PACKED_FORMATS.into_iter().flat_map(move |format| {
                [false, true].into_iter().map(move |dither| PinCase {
                    frame_name,
                    format,
                    quality: Quality::Slow,
                    dither,
                })
            })
        })
        .collect()
}

fn dxt_cases() -> Vec<PinCase> {
    ordinary_frames()
        .into_iter()
        .flat_map(|frame_name| {
            DXT_FORMATS.into_iter().flat_map(move |format| {
                QUALITIES.into_iter().map(move |quality| PinCase {
                    frame_name,
                    format,
                    quality,
                    dither: true,
                })
            })
        })
        .collect()
}

fn degenerate_cases() -> Vec<PinCase> {
    DEGENERATE_FRAMES
        .into_iter()
        .flat_map(|frame_name| {
            DEGENERATE_FORMS
                .into_iter()
                .map(move |(format, quality, dither)| PinCase {
                    frame_name,
                    format,
                    quality,
                    dither,
                })
        })
        .collect()
}

fn check(cases: &[PinCase]) {
    let jobs = available_parallelism();
    let frames: HashMap<&str, Frame> = corpus().into_iter().collect();
    let digests: HashMap<&str, &str> = DIGESTS.iter().copied().collect();
    let mut unrecorded = Vec::new();
    for case in cases {
        let raster = frames.get(case.frame_name).expect("known corpus frame");
        let actual = digest(raster, case.format, case.quality, case.dither, jobs);
        match digests.get(case.key().as_str()) {
            Some(expected) => assert_eq!(*expected, actual, "{}", case.key()),
            None => unrecorded.push(format!("unrecorded digest for {}: {actual}", case.key())),
        }
    }
    assert!(unrecorded.is_empty(), "{}", unrecorded.join("\n"));
}

#[test]
fn packed_formats_reproduce_the_recorded_digests() {
    check(&packed_cases());
}

#[test]
fn dxt_formats_reproduce_the_recorded_digests() {
    check(&dxt_cases());
}

#[test]
fn degenerate_blocks_reproduce_the_recorded_digests() {
    check(&degenerate_cases());
}

#[test]
fn output_does_not_depend_on_the_job_count() {
    let processors = available_parallelism();
    for (name, raster) in corpus64() {
        for (format, quality, dither) in JOB_CASES {
            assert_job_independent(name, &raster, format, quality, dither, 3, processors);
        }
    }
    let raster = corpus()
        .into_iter()
        .find(|(name, _)| *name == "noisy64x64")
        .expect("noisy64x64")
        .1;
    for (format, quality, dither) in VERY_SLOW_JOB_CASES {
        assert_job_independent(
            "noisy64x64",
            &raster,
            format,
            quality,
            dither,
            1,
            processors,
        );
    }
}

fn assert_job_independent(
    name: &str,
    raster: &Frame,
    format: VtfFormat,
    quality: Quality,
    dither: bool,
    repeats: usize,
    processors: usize,
) {
    let label = key(name, format, quality, dither);
    let serial = digest(raster, format, quality, dither, 1);
    assert_eq!(
        serial,
        digest(raster, format, quality, dither, 2),
        "{label} at jobs = 2"
    );
    assert_eq!(
        serial,
        digest(raster, format, quality, dither, processors),
        "{label} at jobs = {processors}"
    );
    for run in 0..repeats {
        assert_eq!(
            serial,
            digest(raster, format, quality, dither, processors),
            "{label} repeat {run}"
        );
    }
}

fn available_parallelism() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
}
