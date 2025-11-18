# =============================================================================
# Smeggdrop Missing Variables Restoration Script
# Generated from analysis of stolen-treasure.tcl
# =============================================================================

# -----------------------------------------------------------------------------
# ARRAYS (variables used with array syntax like $var(key))
# -----------------------------------------------------------------------------

# Adjectives array - keyed by first letter
array set ::adjectives {}
foreach letter {a b c d e f g h i j k l m n o p q r s t u v w x y z} {
    set ::adjectives($letter) [list "awesome" "bad" "cool" "dirty" "evil" "funny" "good" "happy" "interesting" "jolly" "kind" "lazy" "mighty" "nice" "odd" "pretty" "quiet" "rough" "silly" "tiny" "ugly" "vast" "weird" "xtra" "yellow" "zesty"]
}

# Nouns array - keyed by first letter
array set ::nouns {}
foreach letter {a b c d e f g h i j k l m n o p q r s t u v w x y z} {
    set ::nouns($letter) [list "apple" "ball" "cat" "dog" "elephant" "frog" "goat" "horse" "igloo" "jar" "kite" "lamp" "mouse" "nest" "owl" "pig" "queen" "rat" "snake" "tree" "umbrella" "van" "whale" "xray" "yak" "zebra"]
}

# Verbs array - keyed by first letter
array set ::verbs {}
foreach letter {a b c d e f g h i j k l m n o p q r s t u v w x y z} {
    set ::verbs($letter) [list "act" "bump" "cut" "drop" "eat" "fall" "grab" "hit" "jump" "kick" "lick" "move" "nap" "open" "push" "quit" "run" "sit" "talk" "use" "vote" "walk" "yell" "zoom"]
}

# Other arrays - initialize as empty
array set ::acronym {}
array set ::airport {}
array set ::alphabet2 {}
array set ::audio_tagged {}
array set ::butteshacksymbols {}
array set ::canvas {}
array set ::cdown_events {}
array set ::cutbackpix {}
array set ::dancingfont {}
array set ::doletest {}
array set ::eotws {}
array set ::flickrid {}
array set ::http_status_messages {
    200 "OK"
    301 "Moved Permanently"
    302 "Found"
    400 "Bad Request"
    403 "Forbidden"
    404 "Not Found"
    500 "Internal Server Error"
}
array set ::images_tagged {}
array set ::incogstories {}
array set ::lastfmnamemap {}
array set ::lulzscores {}
array set ::myweather {}
array set ::naughtylist {}
array set ::phrases {}
array set ::playlist {}
array set ::profanisaurus {}
array set ::quotes {}
array set ::randacro {}
array set ::slot_stats {}
array set ::smallcap_map {
    A ᴀ B ʙ C ᴄ D ᴅ E ᴇ F ꜰ G ɢ H ʜ I ɪ J ᴊ K ᴋ L ʟ M ᴍ
    N ɴ O ᴏ P ᴘ Q Q R ʀ S ꜱ T ᴛ U ᴜ V ᴠ W ᴡ X x Y ʏ Z ᴢ
}
array set ::spellphabet {
    a alpha b bravo c charlie d delta e echo f foxtrot g golf h hotel
    i india j juliet k kilo l lima m mike n november o oscar p papa
    q quebec r romeo s sierra t tango u uniform v victor w whiskey
    x xray y yankee z zulu
}
array set ::tld {}
array set ::trace_help {}
array set ::twatmap {}
array set ::unicodedescriptivenametable {}
array set ::wu {}
array set ::xboxnames {}

# -----------------------------------------------------------------------------
# DICTIONARY LISTS (lists of variable names for random selection)
# -----------------------------------------------------------------------------

set ::dicts_singleword {
    goon_dict kill_dict faglame_dict insult_dict fetish_dict
    food_dict ethnic_group_dict tranny_dict pedo_dict sex_fluid_dict
    crw_dict gudrow_dict unixprog_dict programming_language_dict
    computer_language_dict canadian_food_dict eastern_european_dict
    goonjob_dict
}

set ::dicts_twowords {
    actor_action
}

# -----------------------------------------------------------------------------
# WORD/PHRASE LISTS (for random selection)
# -----------------------------------------------------------------------------

# People/demographics
set ::arabs {Ahmed Mohamed Hassan Omar Ali Fatima}
set ::asians {Chen Wang Li Zhang Liu Yamamoto Tanaka}
set ::blacks {Tyrone Jamal DeShawn Laquisha Shaniqua}
set ::brits {Nigel Charles Winston Margaret Elizabeth}
set ::canucks {Doug Bob Gordie Wayne Gretsky}
set ::chinees {Wong Chan Lee Kim Park}
set ::dutch {Van Der Berg Bakker Jansen}
set ::finns {Mikkonen Korhonen Virtanen}
set ::gingers {Carrot Top Ronald McDonald Wendy}
set ::nords {Thor Odin Freya Loki}
set ::swedes {Svensson Johansson Andersson}

# Generic word lists
set ::baby {baby infant child tot}
set ::bands {Beatles Stones Nirvana Pearl Jam}
set ::bots {ChanServ NickServ MemoServ}
set ::cheeses {cheddar swiss brie gouda}
set ::cities {New York Los Angeles Chicago Houston}
set ::clothing {shirt pants jacket shoes}
set ::corporations {Microsoft Apple Google Amazon}
set ::creature {dragon unicorn griffin phoenix}
set ::currencies {USD EUR GBP JPY}
set ::databases {MySQL PostgreSQL MongoDB Redis}
set ::demons {Beelzebub Asmodeus Mammon}
set ::directions {north south east west}
set ::dope {weed grass herb chronic}
set ::dune {spice melange sandworm}
set ::emoji {😀 😂 🤣 😊 😍}
set ::emotions {happy sad angry scared}
set ::fluids {water oil blood sweat}
set ::freedoms {speech press assembly religion}
set ::games {chess poker blackjack craps}
set ::gender {male female other}
set ::genre {rock jazz blues country}
set ::girls {Jennifer Jessica Sarah Ashley}
set ::language {English Spanish French German}
set ::lumps {bump lump hump clump}
set ::man {dude guy bro fella}
set ::matter {solid liquid gas plasma}
set ::month {January February March April May June July August September October November December}
set ::nationalities {American British Canadian Australian}
set ::narcotics {heroin cocaine meth}
set ::noradio {silence quiet mute}
set ::norse {Odin Thor Freya Loki}
set ::oneoffs {once twice thrice}
set ::opers {+o +v @}
set ::pig {pig hog swine}
set ::ponies {Twilight Rainbow Pinkie Rarity}
set ::primates {monkey ape gorilla chimp}
set ::relatives {mother father sister brother}
set ::reptoids {lizard snake reptilian}
set ::snow {powder flurry blizzard}
set ::songs {Yesterday Imagine Bohemian}
set ::sources {Wikipedia Google Reddit}
set ::sports {football basketball baseball hockey}
set ::state {California Texas New York Florida}
set ::states {Alabama Alaska Arizona Arkansas California}
set ::stopwords {the a an is are}
set ::subgenre {death black thrash doom}
set ::t {}
set ::thinktanks {RAND Brookings Heritage}
set ::timeunits {second minute hour day week month year}
set ::titles {Mr Mrs Ms Dr Prof}
set ::tlds {com net org edu gov}
set ::toppings {cheese pepperoni mushroom}
set ::tupac {Thug Life All Eyez}
set ::tvshows {Seinfeld Friends Simpsons}
set ::valentines {roses chocolates candy hearts}
set ::voices {soprano alto tenor bass}
set ::vowels {a e i o u}

# Action/activity words
set ::age {young old middle-aged elderly}
set ::annoying {annoying irritating bothersome}
set ::badafterlives {hell purgatory void}
set ::badjectives {terrible horrible awful}
set ::betray_word {betray backstab deceive}
set ::bleep_word {bleep censored redacted}
set ::clothing {shirt pants jacket shoes hat}
set ::cookin_verb_word {fry bake boil grill}
set ::cooking_appliance_word {oven stove microwave}
set ::deadprank {dead deceased departed}
set ::disability {blind deaf mute}
set ::dope {weed grass herb}
set ::dork_quotes {"I'm not a nerd, I'm a geek" "Actually..." "Well, technically..."}
set ::ejaculate_dict {spurt squirt shoot}
set ::eugenics {breeding selection culling}
set ::facebook {like share comment}
set ::fail_list {fail error crash}
set ::feedback {good bad neutral}
set ::feminisms {equality rights choice}
set ::fools {fool idiot moron}
set ::fuck {fuck damn shit}
set ::fuckwords {fuck shit damn}
set ::funfacts {"The mitochondria is the powerhouse of the cell" "Honey never spoils"}
set ::getlow {low down bottom}
set ::girlwords {cute pretty adorable}
set ::goodmeasures {excellent superb fantastic}
set ::googleproducts {Gmail Maps Drive Docs}
set ::greeting_dict {hello hi hey howdy}
set ::happy {happy joyful content}
set ::hate_crime_dict {assault battery harassment}
set ::hatecrimes {assault battery}
set ::hoser {hoser loser}
set ::hotbuttons {politics religion}
set ::hug {hug embrace}
set ::iphone {iPhone iPad iPod}
set ::kill_myself {end it all, check out early}
set ::koran {Quran Koran}
set ::lala {la la la}
set ::language {English Spanish French German Chinese}
set ::last_words {"Tell my wife..." "I regret nothing" "Rosebud"}
set ::lastmeasure {}
set ::linux_sound {ALSA PulseAudio OSS}
set ::man {man guy dude bro}
set ::matter {matter stuff things}
set ::mmm_phrases {delicious tasty yummy}
set ::morsecodemap {A .- B -... C -.-. D -.. E . F ..-. G --. H .... I .. J .--- K -.- L .-.. M -- N -. O --- P .--. Q --.- R .-. S ... T - U ..- V ...- W .-- X -..- Y -.-- Z --..}
set ::nam {Vietnam war conflict}
set ::narcotics {heroin cocaine meth}
set ::nattyism_list {bro lift gains}
set ::nazi {nazi fascist}
set ::negative_dict {bad terrible awful}
set ::ninties {90s nineties}
set ::odin {Odin Allfather}
set ::odinesque {mighty powerful}
set ::omfg {omfg omg wtf}
set ::opinion_dict {think believe feel}
set ::paramsand {and or but}
set ::pc_phrase {politically correct}
set ::pervprefix {dirty nasty}
set ::pet_names {buddy rex max}
set ::phonenumber {555-1234}
set ::phonetic {alpha bravo charlie}
set ::pig {pig hog swine}
set ::pixtag {image photo picture}
set ::please {please kindly}
set ::porn_adj {hot sexy}
set ::porn_noun {video movie clip}
set ::proverbs {"A penny saved is a penny earned" "Early bird gets the worm"}
set ::please {please kindly}
set ::rainbow_taste {fruity colorful}
set ::rps {rock paper scissors}
set ::rufas {}
set ::skifree {ski snow mountain}
set ::slack {slack idle lazy}
set ::smiles {:) :D :P}
set ::socialist_country {Cuba Venezuela}
set ::socialist_word {comrade solidarity}
set ::stallman_interject {"I'd just like to interject for a moment"}
set ::steveism {"Stay hungry, stay foolish"}
set ::stopwords {the a an and or}
set ::strain_adjective {purple sour}
set ::strain_location {Afghan Kush}
set ::strain_name {OG Kush Blue Dream}
set ::stupid_dict {stupid dumb idiotic}
set ::subgenre {death black thrash}
set ::suicide_method {}
set ::terror_phrases {bomb attack threat}
set ::timeunits {second minute hour day}
set ::tinfoilhats {conspiracy theory}
set ::titles {Mr Mrs Ms Dr}
set ::ubuntu_verb {sudo apt-get}
set ::upgradephrase {upgrade update}
set ::valentines {roses chocolates hearts}
set ::virus_phrases {infected malware trojan}
set ::viscosity {thick thin}
set ::vital_aircraft_part {wing engine}
set ::voices {soprano alto tenor bass}
set ::vowels {a e i o u}
set ::weatherconditions {sunny cloudy rainy}
set ::weatherwords {weather forecast}
set ::wheelchair {wheelchair handicap}
set ::wizard_quotes {"A wizard is never late"}
set ::woman_parts {hair face body}
set ::worthless_degree {philosophy art history}
set ::ww_item {item thing}
set ::zalgo {}

# -----------------------------------------------------------------------------
# NUMERIC/COUNTER VARIABLES
# -----------------------------------------------------------------------------

set ::btc_last 0
set ::btc_prev 0
set ::ceval_max_entries 100
set ::ceval_ttl 300
set ::gay_count 0
set ::gypsy_count 0
set ::happy_n 0
set ::imdbprevsize 0
set ::jew_count 0
set ::lind_last 0
set ::lind_prev 0
set ::max_poopers 10
set ::mikes_count 0
set ::nord_count 0
set ::old_count 0
set ::republican_count 0
set ::safetyimagelimit 100
set ::stoner_count 0
set ::toke_countdown_n 0
set ::triv_distance 0
set ::twink_count 0

# Roulette game state
set ::fedroulette_current_chamber 1
set ::fedroulette_n_chambers 6

# -----------------------------------------------------------------------------
# CACHED VALUES (typically empty until populated)
# -----------------------------------------------------------------------------

set ::cached_ {}
set ::cached_ScheisseGern {}
set ::cached_barjoke {}
set ::cached_bike {}
set ::cached_fart {}
set ::cached_hillary {}
set ::cached_hurfle {}
set ::cached_lolqdb {}
set ::cached_obama {}
set ::cached_onehug {}
set ::cached_sfart {}
set ::cached_troll_me_raw {}
set ::cached_vladfarted {}
set ::cached_waffleimages {}

# -----------------------------------------------------------------------------
# SPECIAL/COMPLEX LISTS
# -----------------------------------------------------------------------------

# Incogagenda - agenda items with substitution patterns
set ::incogagenda {
    {go to work}
    {buy groceries}
    {call mom}
    {fix bug in code}
    {attend meeting}
    {watch TV}
}

# lmoot ASCII art
set ::lmoot_ascii {
    {  __________}
    { /          \\}
    {|  BUTTES   |}
    {|   CHAT    |}
    { \\__________/}
}

# Various lookup tables and maps
set ::flipmap {}
set ::vflip_pairs {}
set ::ligature_map {}
set ::fraktur_abuse_map {}
set ::unicode_abuse_map {}
set ::rand_wordin_map {}
set ::kallebooize_map {}
set ::german_translation_key {}
set ::flippedomgdude {}

# Sentence/template variables
set ::acro_template {[adj][noun]}
set ::asciiregex {[\x00-\x7F]}
set ::twitter_regexp {@[a-zA-Z0-9_]+}
set ::winfoboxregex {}
set ::wu_regexp {}
set ::wu_regexp_rss {}
set ::yt_search_regexp {}

# URL/host patterns
set ::pastehost {pastebin.com}
set ::shorturls {bit.ly tinyurl.com}
set ::wurl {}

# -----------------------------------------------------------------------------
# GAME/TRIVIA STATE
# -----------------------------------------------------------------------------

set ::blackjack_deck {}
set ::current_game {}
set ::thegame {}
set ::trivia_answer {}
set ::trivia_cat {}
set ::trivia_last {}

# -----------------------------------------------------------------------------
# DICTIONARY-TYPE VARIABLES (_dict suffix)
# These contain themed word/phrase lists for random selection
# -----------------------------------------------------------------------------

set ::MCPU_dict {CPU processor chip}
set ::albot_dict {robot bot ai}
set ::albotspew_dict {spam flood}
set ::antivirus_dict {Norton McAfee Kaspersky}
set ::ass_objects_dict {chair bench stool}
set ::bb_dict {bb gun pellet}
set ::blackname_dict {Tyrone DeShawn Jamal}
set ::body_dict {arm leg head torso}
set ::britword_dict {colour favourite honour}
set ::buttes_cat_dict {Whiskers Mittens Fluffy}
set ::buttes_dog_dict {Rex Spot Fido}
set ::buttes_poems_dict {"Roses are red"}
set ::canadaword_dict {eh sorry hockey}
set ::cartoon_show_dict {Simpsons Futurama}
set ::cat_battle_dict {scratch hiss meow}
set ::cat_dict {tabby siamese persian}
set ::chingchong_dict {ching chong}
set ::chinkplace_dict {China Taiwan}
set ::chucknorris_dict {"Chuck Norris counted to infinity. Twice."}
set ::comedian_dict {Carlin Pryor Chappelle}
set ::comment_word_dict {wow amazing great}
set ::condition_dict {good bad fair}
set ::country_name_dict {America Canada Mexico}
set ::crappy_restaurant_dict {Arbys Dennys}
set ::daypart_dict {morning afternoon evening}
set ::deity_dict {God Allah Buddha}
set ::device_dict {phone laptop tablet}
set ::diagnosis_dict {flu cold virus}
set ::dict_famousniggermen {MLK Malcolm X}
set ::dict_famousniggerwomen {Rosa Parks Oprah}
set ::dikkyizedict {}
set ::disease_dict {cancer aids flu}
set ::disease_subst_dict {disease illness}
set ::dog_dict {labrador poodle bulldog}
set ::drug_dict {weed coke meth}
set ::dumbest_dict {dumbest stupidest}
set ::english_meal_dict {breakfast lunch dinner}
set ::era_dict {medieval modern}
set ::ethnic_cleansing_dict {}
set ::face_dict {eyes nose mouth}
set ::fake_italian_place {Tuscany Roma}
set ::familyguy_chardict {Peter Lois Stewie}
set ::famous_gbs_dict {}
set ::fastfood_dict {McDonalds Wendys BurgerKing}
set ::fatmouse_dict {}
set ::frenchword_dict {oui non merci}
set ::fuck_dict {fuck shit damn}
set ::fundie_dict {bible jesus church}
set ::furry_noun_dict {fur tail paw}
set ::furry_verb_dict {yiff nuzzle}
set ::gbs_aim_dict {}
set ::generic_insult_dict {idiot moron}
set ::genre_dict {rock pop jazz}
set ::goon_fix_dict {fix repair patch}
set ::goonphrase_dict {}
set ::goony_dict {}
set ::hate_crime_dict {assault battery}
set ::hightax_dict {}
set ::hod_dict {}
set ::hole_dict {hole pit void}
set ::honourary_dict {honorary}
set ::hzu_dict {}
set ::internet_dict {web net cyber}
set ::internetacro_dict {lol rofl lmao}
set ::ircnet_dict {freenode efnet}
set ::japanese_thing_dict {anime manga sushi}
set ::jewboy_activity_dict {}
set ::jewproduct_dict {}
set ::jewword_dict {}
set ::job_dict {doctor lawyer teacher}
set ::kidpornstar_dict {}
set ::lamejoke_dict {"Why did the chicken cross the road?"}
set ::liquid_dict {water juice milk}
set ::lisp_dict {car cdr cons}
set ::literati_dict {}
set ::loco_dict {crazy loco}
set ::lorem_dict {lorem ipsum dolor}
set ::mahvel_dict {}
set ::marginalize_dict {}
set ::mobile_prefix_dict {smart cell}
set ::moira_dict {}
set ::mystic_dict {mystic magic}
set ::niggerboy_dict {}
set ::niggerfriend_dict {}
set ::niggerlastname_dict {Washington Jefferson}
set ::octalemo_dict {}
set ::okey_defense_dict {}
set ::opamp_dict {}
set ::oregon_trail_disease_dict {dysentery cholera}
set ::os_dict {Windows Linux Mac}
set ::overused_dict {literally basically}
set ::penis_dict {}
set ::politigoon_finale_dict {}
set ::popeye_dict {spinach Olive Bluto}
set ::porn_scenes_dict {}
set ::ppc_dict {}
set ::pretty_girl_dict {beautiful gorgeous}
set ::promote_dict {promote advertise}
set ::pua_anecdotes {}
set ::puntme_dict {}
set ::quiz_dict {}
set ::raw_vegan_dict {salad kale}
set ::resist_dict {resist fight}
set ::rolloffle_annoy_dict {}
set ::scieno_dict {Scientology Xenu}
set ::sex_do_dict {}
set ::sextoy_dict {}
set ::shakti_adj_dict {}
set ::shithead_dict {idiot moron}
set ::shsc_thread_dict {}
set ::sing_ethnic_group_dict {}
set ::slur_dict {}
set ::small_dict {tiny little mini}
set ::small_stamp_dict {}
set ::song_front_dict {}
set ::song_modifier_dict {}
set ::spook_dict {ghost spirit}
set ::std_dict {herpes chlamydia}
set ::stig_dict {}
set ::taps_wi_dict {}
set ::teapot_dict {teapot kettle}
set ::texture_dict {smooth rough}
set ::tf2_weapon_dict {rocket shotgun}
set ::tfr_dict {}
set ::theo_dict {}
set ::time_dict {now later soon}
set ::train_line_dict {metro subway}
set ::trannies_dict {}
set ::transport_dict {car bus train}
set ::utd_aim_dict {}
set ::victim_dict {victim target}
set ::viet_name_dict {Nguyen Tran}
set ::vulnerability_dict {}
set ::weapon_dict {gun knife sword}
set ::wi_flag_dict {}
set ::winprog_dict {notepad calc}
set ::zapanig_dict {}
set ::zybl0re_dict {}

# -----------------------------------------------------------------------------
# ASCII ART AND BANNERS
# -----------------------------------------------------------------------------

set ::ENNbanner {=== ENN NEWS ===}
set ::SIGBART {SIGBART}
set ::STbrr {brr}
set ::STsonic_lines {}
set ::TAFKADH {}
set ::____arrays {}
set ::alligatormanstamp {}
set ::bart {}
set ::bgbl {}
set ::bigdrippingcock {}
set ::bonercry_macro {}
set ::butteslogo {BUTTES}
set ::catbus {}
set ::chatzillastamp {}
set ::cockbuttes {}
set ::colormooninite {}
set ::delorean {}
set ::gaydารascii {}
set ::gridse {}
set ::huge_moira_image {}
set ::imgaflipcanvas {}
set ::iraqipig {}
set ::kidpixoverlays {}
set ::leftomgchair {}
set ::letmeinonthisascii {}
set ::mandel1 {}
set ::mandel2 {}
set ::moira5 {}
set ::nethacksymbols {}
set ::omgdude {}
set ::omgwebcam {}
set ::opensslmacro {}
set ::outlines {}
set ::overlays {}
set ::poostamp {}
set ::rightomgchair {}
set ::sadball {}
set ::sexoffendercard {}
set ::showerstamp {}
set ::snoopy {}
set ::totorostamp {}
set ::tuxpaintstamps {}
set ::twobyeightmouthes {}
set ::unhappybat {}
set ::weed_ascii {}
set ::winkie_macro {}

# -----------------------------------------------------------------------------
# REMAINING LISTS AND SPECIAL VARIABLES
# -----------------------------------------------------------------------------

set ::GoonGym {gym workout lift}
set ::Grog {grog rum}
set ::LK_fav {}
set ::TVs {Samsung LG Sony}
set ::_brony {}
set ::abez_word {}
set ::abezeyes {}
set ::abeznoses {}
set ::acquaintances {}
set ::affirmative_statements {yes sure okay}
set ::african_capitals {Cairo Lagos Nairobi}
set ::aim_dicts {}
set ::aimaway {}
set ::aimhax {}
set ::aimpranks {}
set ::aimsaq {}
set ::al_nig {}
set ::alert {}
set ::andrzej_eyes {}
set ::andrzej_mouth {}
set ::andrzej_nose {}
set ::animalprefix {dog cat bird}
set ::animulist {anime manga}
set ::aplus_lines {}
set ::appliancejoke {}
set ::arcade_font {}
set ::asciiartfartsids {}
set ::asciiartfartsidstxttime 0
set ::asciioftheweek {}
set ::askee2 {}
set ::ass_verb_past_tense {sat laid}
set ::asstory {}
set ::audiophile_brands {Bose Sennheiser}
set ::beastmovie {}
set ::biblebooks {Genesis Exodus Leviticus}
set ::biblical {Adam Eve Moses}
set ::bigmatixtesturls {}
set ::bike_pols {}
set ::bikefags {}
set ::black_figures {MLK Obama}
set ::black_star_trek_characters {Uhura Geordi Worf}
set ::blackfacts {}
set ::blackitem {}
set ::blake_qualification {}
set ::blog_starter_phrases {"Today I want to talk about"}
set ::blogentrywords1 {amazing incredible}
set ::blogentrywords2 {journey adventure}
set ::bsl {}
set ::bullmonkey_phrases {}
set ::buttesclassified {}
set ::buttesforts {}
set ::butteshackgems {}
set ::butteshackgraves {}
set ::butteshackwands {}
set ::buttesread {}
set ::buttesrooms {}
set ::c_lisp {}
set ::caco {}
set ::canada_cities {Toronto Vancouver Montreal}
set ::canadian_objects {hockey maple syrup}
set ::car_brand {Ford Toyota Honda}
set ::catbus_word {}
set ::cdma {}
set ::certs {SSL TLS}
set ::changingmeasures {}
set ::chat_lines {}
set ::chile_colours {red white blue}
set ::chirpz {}
set ::chirpz_word {}
set ::chocolaterain {}
set ::churl {}
set ::citysuffixes {ville town city}
set ::clapperlist {}
set ::clippy_lines {"It looks like you're writing a letter"}
set ::cnbc_symbols {AAPL GOOG MSFT}
set ::cocke_headlines {}
set ::colinagenda {}
set ::colinhates {}
set ::collegeaim {}
set ::comicstyles {}
set ::conspiracy_groups {Illuminati NWO}
set ::convol {}
set ::coolcontest {}
set ::crw_phrase {}
set ::crw_verb {}
set ::crypto_words {bitcoin blockchain}
set ::currency_map {}
set ::cyber_database {}
set ::cyberwarfags_symbols {}
set ::d_as {}
set ::darfur_lines {}
set ::darren {}
set ::darren_word {}
set ::darrenlist {}
set ::debian_stable_features {}
set ::dikky_word {}
set ::dndattribute {strength dexterity constitution}
set ::doctor_mouths {}
set ::dolphin_phrases {}
set ::dprk_slogan {}
set ::dprk_to_buttes {}
set ::dr_greeting {Hello patient}
set ::drill_lines {}
set ::drudge_headlines {}
set ::drug_does {}
set ::drug_doings {}
set ::dukekusmess {}
set ::dumbstates {}
set ::ebay_adjectives {rare vintage}
set ::ebuild_categories {}
set ::ed209_words {}
set ::electionpranks {}
set ::emacslam {}
set ::emoticons {:) :( :P}
set ::engimodict {}
set ::englishtownsuffices {shire ham}
set ::espresso_drink {latte cappuccino}
set ::ethnic_food_people {}
set ::familyguy_database {}
set ::farklines {}
set ::fart_lines {}
set ::fat_pua_anecdotes {}
set ::fatjokes {}
set ::fatquestion {}
set ::favouritejb {}
set ::fecal_shapes {}
set ::figfonts {}
set ::fml_images {}
set ::fml_today_words {}
set ::fone_function {}
set ::fone_models {iPhone Galaxy}
set ::fone_names {}
set ::fontsizearial16 {}
set ::forecast_locs {}
set ::former_countries {USSR Yugoslavia}
set ::freedom_words {liberty freedom}
set ::frogstat {}
set ::frotteur_quotes {}
set ::fsjism {}
set ::fundie_phrases {}
set ::gamerwords {noob pwned}
set ::gangcopyright {}
set ::ganghosts {}
set ::gay_randoms {}
set ::gayagenda {}
set ::gaychorus {}
set ::gayflag_bg {}
set ::gayflag_fg {}
set ::gbl {}
set ::goon_cooking_method {fry boil grill}
set ::goon_liquidqty {}
set ::goon_shakti {}
set ::goon_solidqty {}
set ::goonhost {}
set ::hackernews_tech_word {blockchain AI}
set ::hello_procs {}
set ::hindudot {}
set ::historical_events {"Moon landing" "Fall of Berlin Wall"}
set ::holohoax_questions {}
set ::icns {}
set ::image_format {jpg png gif}
set ::imageshack {}
set ::incog_word {}
set ::indian_cities {Mumbai Delhi}
set ::indiannames {Raj Priya}
set ::infowars {}
set ::insulting_dict {}
set ::internet_devices {router modem}
set ::ipoddata {}
set ::italian_first_name_f {Maria Sofia}
set ::italian_first_name_m {Marco Luigi}
set ::italian_last_name {Rossi Russo}
set ::jabon_lines {}
set ::jap_emotes {}
set ::japan_resident {}
set ::jarlinks {}
set ::jerkcitydb {}
set ::jewesses {}
set ::jgirlfirstnames {}
set ::jgirllastnames {}
set ::jones_ {}
set ::jrelol {}
set ::kalleboo_mroach_dict {}
set ::kallecleanlines {}
set ::kallefront_gems {}
set ::kellyphrase {}
set ::khan_quotes {"KHAAAN!"}
set ::kim_il_sung_quotes {}
set ::kim_jong_il_quotes {}
set ::kkkchat {}
set ::km_place {}
set ::lastfm_cockes_usermap {}
set ::lastfm_user_map {}
set ::lastfmtimeouttime 0
set ::learning_book_title_prefix {Learn Mastering}
set ::learning_book_title_suffix {"in 24 Hours" "for Dummies"}
set ::leasers_list {}
set ::leftmost {}
set ::legendtitles {}
set ::liberal_newspaper {NY Times Washington Post}
set ::liberal_tv {CNN MSNBC}
set ::liberal_website {Huffington Post}
set ::liberalagenda {}
set ::liberty_points {}
set ::likeabosslyrics {}
set ::likeacat {}
set ::lispmonster {}
set ::lispquote {}
set ::lolqdb_lines {}
set ::lulzball {}
set ::mac_apps {Safari Mail}
set ::mac_fix {}
set ::macpranks {}
set ::malcolmtest2013may20 {}
set ::maria_lines {}
set ::mathjoke {}
set ::mba_phrases {}
set ::measurement_units {meter kilogram second}
set ::mecountry {}
set ::medical_names {Dr Smith}
set ::medical_press {}
set ::medical_specialties {cardiology neurology}
set ::medical_title {MD PhD}
set ::menu_drink {soda coffee tea}
set ::menu_food_prefix {super mega}
set ::menu_food_suffix {burger wrap}
set ::mexicanfood {taco burrito}
set ::mexico_colours {green white red}
set ::microsoft_domains {microsoft.com outlook.com}
set ::midgets_hang {}
set ::miscjre {}
set ::modhash {}
set ::moira_let {}
set ::movieseasons {}
set ::mroach_item {}
set ::mroach_obsession {}
set ::mroach_word {}
set ::msnbc_subject {}
set ::music_genres {rock pop jazz}
set ::musicartists {Beatles Stones}
set ::news_network {CNN Fox MSNBC}
set ::next_iphone_ver 15
set ::next_osx_ver 14
set ::nicklist {}
set ::nigerian_spam_lines {}
set ::niggerdislikes {}
set ::niggerize {}
set ::niggerlikes {}
set ::niggerphrases {}
set ::niggersentence {}
set ::niggerwomen {}
set ::niggerwords {}
set ::now_thats_what_i_call_autism {}
set ::nwo_chemicals {}
set ::nwoyoutube {}
set ::ny_locales {Manhattan Brooklyn}
set ::obama {}
set ::obama_response {}
set ::obamarope {}
set ::offender_type {}
set ::okey {}
set ::okey_act {}
set ::okey_acts {}
set ::okey_specattack {}
set ::okey_spell {}
set ::okey_types {}
set ::oldtest_insult {}
set ::onebutan_lastfm_usermap {}
set ::onebutan_rec {}
set ::onebutan_recommendation {}
set ::onebutan_topic {}
set ::onnotice_list {}
set ::oolist {}
set ::ourdevshit {}
set ::ourshit {}
set ::overheard_lines {}
set ::overheard_people {}
set ::penis_song_lyrics {}
set ::periodic_metals {gold silver iron}
set ::perl_module {strict warnings}
set ::piza_toppings {cheese pepperoni}
set ::political_figures {Biden Trump}
set ::politicalstance {}
set ::pornstars {}
set ::ppc_krew {}
set ::prankideas {}
set ::precepts {}
set ::psych1 {}
set ::psych2 {}
set ::psych3 {}
set ::pua_adjective {}
set ::pua_locations {}
set ::pua_object {}
set ::pua_verb {}
set ::pua_verb2 {}
set ::publications {TIME Newsweek}
set ::pukeonopers {}
set ::puntme_phrases {}
set ::puterlanguage {Python Java}
set ::r1ch_line {}
set ::racism_tuples {}
set ::radio_slogans {}
set ::randart_trans {}
set ::rbl_quote_procs {}
set ::rcookie {}
set ::re_flags {}
set ::reddit_currencies {}
set ::religion_adherant {}
set ::resumelines {}
set ::ripper {}
set ::rorschach_response {}
set ::rotB {}
set ::rotC {}
set ::rotOff {}
set ::rotV {}
set ::rotflmao {}
set ::rrl {}
set ::rule_of_acquisition {"Once you have their money, never give it back"}
set ::rumored_apple_products {}
set ::sandworm {}
set ::santa_deadbeats {}
set ::saq_beer_lines {}
set ::saq_beer_review_map {}
set ::saq_expert_topics {}
set ::saq_girls {}
set ::saq_skills {}
set ::sexparts {}
set ::sextoys {}
set ::shitty_company {}
set ::shsc_anecdotes {}
set ::shsc_experience {}
set ::sine2duration {}
set ::sine2pitch {}
set ::sine2volume {}
set ::sjw_arguments {}
set ::sjwfreud {}
set ::slot_jackpot 0
set ::spankbank {}
set ::specialinterest {}
set ::spitroast {}
set ::spookhint {}
set ::srsagenda {}
set ::stool_description {}
set ::stoollookup {}
set ::strategygoon {}
set ::strid_word {}
set ::suckadicksong {}
set ::svslolnick {}
set ::swede_city {Stockholm Malmo}
set ::swede_first {Erik Lars}
set ::swede_last {Svensson Johansson}
set ::swingler_list_action {}
set ::talking_patterns {}
set ::tdi_driver {}
set ::thegaydar {}
set ::thosedevfuckers {}
set ::thosefuckers {}
set ::tinyfugue {}
set ::tl {}
set ::tmsg {}
set ::toronto_names_pool {}
set ::train_status {}
set ::trippin_balls {}
set ::trump_evidence {}
set ::tsapi {}
set ::ttpos_ {}
set ::ttree_ {}
set ::tweeters {}
set ::twitterceleb {}
set ::uarrows_left {←}
set ::uarrows_right {→}
set ::ufc_attack {}
set ::uli_quotes {}
set ::ultralump {}
set ::undesirable_nouns {}
set ::unix_commands {ls cd grep}
set ::unix_path_dict {}
set ::unlimited_plan {}
set ::url_pairs {}
set ::usdevfolks {}
set ::use_flags {}
set ::usfolks {}
set ::vw_tdi_model {}
set ::vxp_colours {}
set ::washington_place {}
set ::wasp_names {Chad Brad}
set ::wasp_surname {Smith Jones}
set ::web2_jobs {}
set ::web2_list {}
set ::what_is_with_fox_news {}
set ::wibdistance 0
set ::windows_points {}
set ::winkie_words {}
set ::wmfprank {}
set ::wmfs {}
set ::wntd_object {}
set ::wntd_subject {}
set ::wow_class {warrior mage}
set ::wow_race {human orc elf}
set ::wow_raid {}
set ::wow_talents {}
set ::xmass_2014 {}
set ::xss {}
set ::xssnimp {}
set ::zubkatz_lines {}
set ::zulu_name {}

# -----------------------------------------------------------------------------
# END OF RESTORATION SCRIPT
# -----------------------------------------------------------------------------

puts "Missing variables restored successfully!"
