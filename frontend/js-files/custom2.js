// jQuery Initialization
jQuery(document).ready(function($){
"use strict"; 


        /* =================================
        ===  MAILCHIMP                 ====
        =================================== */

        $('.mailchimp').ajaxChimp({
            callback: mailchimpCallback,
            url: "http://pixfort.us10.list-manage.com/subscribe/post?u=aeee190b52c1942e89defdb3e&amp;id=c9ca7e0189" //Replace this with your own mailchimp post URL. Don't remove the "". Just paste the url inside "".  
        });

        function mailchimpCallback(resp) {
             if (resp.result === 'success') {
                $('.subscription-success').html('<div class="success">' + resp.msg+ '</div>').fadeIn(1000);
                $('.subscription-error').fadeOut(500);
                
            } else if(resp.result === 'error') {
                $('.subscription-error').html('<div class="error">' + resp.msg + '</div>').fadeIn(1000);
            }  
        }



        //======================================================================================================
        //  END OF DOCUMENT
        //=================
});